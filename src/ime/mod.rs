mod keymap;
mod protocol;
mod state;

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use async_channel::Sender;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::protocol::wl_seat;
use wayland_client::{Connection, EventQueue, globals::registry_queue_init};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;

use self::state::ImeState;
use crate::messages::{UiCommand, UiEvent};

fn debug_enabled() -> bool {
    static DEBUG: OnceLock<bool> = OnceLock::new();
    *DEBUG.get_or_init(|| std::env::var_os("WAYMOJI_DEBUG").is_some())
}

pub(super) fn debug_log(args: std::fmt::Arguments<'_>) {
    if debug_enabled() {
        eprintln!("[waymoji] {args}");
    }
}

fn setup(tx: Sender<UiEvent>) -> Result<(Connection, EventQueue<ImeState>, ImeState)> {
    let conn = Connection::connect_to_env().context("failed to connect to Wayland")?;
    let (globals, mut queue) =
        registry_queue_init::<ImeState>(&conn).context("failed to initialize Wayland globals")?;
    let qh = queue.handle();

    let seat: wl_seat::WlSeat = globals
        .bind(&qh, 1..=9, ())
        .context("failed to bind wl_seat")?;
    let manager: ZwpInputMethodManagerV2 = globals
        .bind(&qh, 1..=1, ())
        .context("compositor does not support zwp_input_method_manager_v2")?;
    let vk_manager: Option<ZwpVirtualKeyboardManagerV1> = globals.bind(&qh, 1..=1, ()).ok();

    let ime = manager.get_input_method(&seat, &qh, ());
    let mut state = ImeState::new(tx, ime, seat, vk_manager);

    queue.roundtrip(&mut state)?;
    debug_log(format_args!(
        "ime setup: active={} grabbed={}",
        state.active,
        state.xkb_state.is_some()
    ));
    Ok((conn, queue, state))
}

fn wait_for_events(
    conn: &Connection,
    queue: &mut EventQueue<ImeState>,
    state: &mut ImeState,
    timeout_ms: i32,
) -> Result<()> {
    queue.dispatch_pending(state)?;

    if let Some(guard) = queue.prepare_read() {
        let timeout = Timespec {
            tv_sec: (timeout_ms / 1000) as i64,
            tv_nsec: ((timeout_ms % 1000) as i64) * 1_000_000,
        };
        let mut fds = [PollFd::new(conn, PollFlags::IN)];
        let ready = match poll(&mut fds, Some(&timeout)) {
            Ok(ready) => ready,
            Err(err) if err == rustix::io::Errno::INTR => return Ok(()),
            Err(err) => return Err(err).context("polling the Wayland socket failed"),
        };
        if ready > 0 && fds[0].revents().contains(PollFlags::IN) {
            match guard.read() {
                Ok(_) => {}
                Err(wayland_client::backend::WaylandError::Io(err))
                    if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err).context("reading Wayland events failed"),
            }
        } else {
            drop(guard);
        }
    }

    queue.dispatch_pending(state)?;
    Ok(())
}

fn drain_wake(wake: &UnixStream) -> Result<()> {
    let mut buffer = [0u8; 64];
    loop {
        match (&*wake).read(&mut buffer) {
            Ok(0) => break,
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err).context("reading wake pipe failed"),
        }
    }
    Ok(())
}

fn wait_for_io(
    conn: &Connection,
    wake: &UnixStream,
    queue: &mut EventQueue<ImeState>,
    state: &mut ImeState,
) -> Result<()> {
    queue.dispatch_pending(state)?;
    conn.flush()?;

    let guard = queue.prepare_read();
    let mut fds = [
        PollFd::new(conn, PollFlags::IN),
        PollFd::new(wake, PollFlags::IN),
    ];

    let ready = match poll(&mut fds, None) {
        Ok(ready) => ready,
        Err(err) if err == rustix::io::Errno::INTR => return Ok(()),
        Err(err) => return Err(err).context("polling Wayland and wake pipes failed"),
    };
    let _ = ready;

    if fds[1].revents().contains(PollFlags::IN) {
        drain_wake(wake)?;
    }
    if fds[0].revents().contains(PollFlags::IN) {
        if let Some(guard) = guard {
            match guard.read() {
                Ok(_) => {}
                Err(wayland_client::backend::WaylandError::Io(err))
                    if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err).context("reading Wayland events failed"),
            }
        }
    } else {
        drop(guard);
    }

    if fds[0]
        .revents()
        .intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL)
    {
        bail!("Wayland connection closed");
    }

    queue.dispatch_pending(state)?;
    Ok(())
}

fn request_grab(
    conn: &Connection,
    queue: &mut EventQueue<ImeState>,
    state: &mut ImeState,
    ignore_after: Duration,
) -> Result<()> {
    let qh = queue.handle();
    if let Some(ime) = state.ime.clone() {
        let grab = ime.grab_keyboard(&qh, ());
        state.grab = Some(grab);
    }
    state.ignore_until = Instant::now() + ignore_after;
    conn.flush()?;
    queue.roundtrip(state)?;
    Ok(())
}

pub fn run(tx: Sender<UiEvent>, rx: Receiver<UiCommand>, wake: UnixStream) {
    if let Err(err) = run_inner(&tx, &rx, wake) {
        let _ = tx.try_send(UiEvent::Error(err.to_string()));
    }
}

fn run_inner(tx: &Sender<UiEvent>, rx: &Receiver<UiCommand>, wake: UnixStream) -> Result<()> {
    let (conn, mut queue, mut state) = setup(tx.clone())?;
    let activation_deadline = Instant::now() + Duration::from_millis(200);
    while !state.active {
        let now = Instant::now();
        if now >= activation_deadline {
            break;
        }
        let remaining = (activation_deadline - now)
            .as_millis()
            .min(i32::MAX as u128) as i32;
        wait_for_events(&conn, &mut queue, &mut state, remaining)?;
    }
    if state.unavailable {
        bail!("another input method is already active on this seat");
    }
    let ignore_after = if state.active {
        Duration::ZERO
    } else {
        Duration::from_millis(50)
    };
    request_grab(&conn, &mut queue, &mut state, ignore_after)?;
    let active = state.active;
    let grabbed = state.xkb_state.is_some();
    debug_log(format_args!(
        "ime ready: active={} grabbed={}",
        active, grabbed
    ));
    let _ = tx.try_send(UiEvent::Ready { active, grabbed });

    loop {
        match rx.try_recv() {
            Ok(UiCommand::Insert(text)) => {
                state.insert(text, &conn, &mut queue)?;
                let _ = tx.try_send(UiEvent::Inserted);
            }
            Ok(UiCommand::Cancel) => {
                state.cancel(&conn);
                return Ok(());
            }
            Err(TryRecvError::Disconnected) => {
                state.cancel(&conn);
                return Ok(());
            }
            Err(TryRecvError::Empty) => {}
        }

        if state.unavailable {
            bail!("another input method is already active on this seat");
        }

        wait_for_io(&conn, &wake, &mut queue, &mut state)?;
    }
}
