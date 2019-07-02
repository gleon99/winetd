mod error;
mod service_listener;

use self::error::Error;
use config;
use log::error;
use mio::{Events, Poll, PollOpt, Ready, Token};
use mio_extras::channel;
use service_listener::ServiceListener;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::mem;
use std::path::PathBuf;
use std::time::Duration;
use winapi;
use winapi::um::winsock2;
use windows_service;
use windows_service::service;
use windows_service::service_control_handler;
use winlog;

windows_service::define_windows_service!(service_entry_point, service_main);

const SERVICE_NAME: &str = "Winet";
const SERVICE_TYPE: service::ServiceType = service::ServiceType::OWN_PROCESS;

pub type Result<T, E = error::Error> = std::result::Result<T, E>;

fn initialize(poll: &Poll) -> Result<HashMap<Token, ServiceListener>> {
    let mut listeners = HashMap::new();
    match env::var("ProgramData") {
        Ok(path) => {
            let mut path = PathBuf::from(path);
            path.push("Winetd");
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let mut settings = config::Config::default();
                settings.merge(config::File::from(entry.path()))?;
                let listener = ServiceListener::new(
                    settings.get_int("port")? as u16,
                    settings.get_str("command")?,
                )?;
                let token = Token(listeners.len() + 1);
                poll.register(
                    listener.get_tcp_listener(),
                    token,
                    Ready::readable(),
                    PollOpt::edge(),
                )?;
                listeners.insert(token, listener);
            }
            Ok(listeners)
        }
        _ => Err(Error::EnvError),
    }
}

fn run(stop_recv: channel::Receiver<()>) -> Result<()> {
    winlog::init("Winetd")?;
    let poll = Poll::new()?;
    let listeners = initialize(&poll)?;
    let mut wsa_data: winsock2::WSADATA = unsafe { mem::zeroed() };
    let result = unsafe { winsock2::WSAStartup(0x202, &mut wsa_data) };
    if result != 0 {
        return Err(Error::WSAStartupError { error: result });
    }

    let mut events = Events::with_capacity(10);

    poll.register(&stop_recv, Token(0), Ready::readable(), PollOpt::edge())?;

    loop {
        poll.poll(&mut events, None)?;
        for event in &events {
            if event.token() == Token(0) {
                return Ok(());
            }
            listeners.get(&event.token()).unwrap().handle_connection()?;
        }

        match stop_recv.try_recv() {
            Ok(()) => return Ok(()),
            _ => continue,
        }
    }
}

fn service_main(_arguments: Vec<OsString>) {
    let (stop_send, stop_recv) = channel::channel::<()>();
    let event_handler =
        move |control_event| -> service_control_handler::ServiceControlHandlerResult {
            match control_event {
                service::ServiceControl::Interrogate => {
                    service_control_handler::ServiceControlHandlerResult::NoError
                }
                service::ServiceControl::Stop => {
                    stop_send.send(()).unwrap();
                    service_control_handler::ServiceControlHandlerResult::NoError
                }
                _ => service_control_handler::ServiceControlHandlerResult::NotImplemented,
            }
        };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler).unwrap();
    status_handle
        .set_service_status(service::ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: service::ServiceState::Running,
            controls_accepted: service::ServiceControlAccept::STOP,
            exit_code: service::ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
        })
        .unwrap();

    if let Err(e) = run(stop_recv) {
        error!("Error: {}", e);
    }

    status_handle
        .set_service_status(service::ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: service::ServiceState::Stopped,
            controls_accepted: service::ServiceControlAccept::empty(),
            exit_code: service::ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
        })
        .unwrap();
}

fn main() -> Result<(), windows_service::Error> {
    windows_service::service_dispatcher::start(SERVICE_NAME, service_entry_point)?;
    Ok(())
}
