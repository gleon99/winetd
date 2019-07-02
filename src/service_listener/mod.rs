use super::error::Error;
use super::Result;
use log::{debug, info};
use mio::net::TcpListener;
use std::ffi::CString;
use std::mem;
use std::net::{Ipv4Addr, SocketAddr};
use std::os::windows::io::IntoRawSocket;
use std::ptr;
use winapi::shared::minwindef::{DWORD, LPCVOID};
use winapi::um::winnt::HANDLE;
use winapi::um::{
    fileapi, handleapi, minwinbase, namedpipeapi, processthreadsapi, winbase, winsock2,
};

fn create_pipe() -> Result<(HANDLE, HANDLE)> {
    static mut SECURITY_ATTRIBUTES: minwinbase::SECURITY_ATTRIBUTES =
        minwinbase::SECURITY_ATTRIBUTES {
            nLength: mem::size_of::<minwinbase::SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: ptr::null_mut(),
            bInheritHandle: true as i32,
        };

    let mut read: HANDLE = ptr::null_mut();
    let mut write: HANDLE = ptr::null_mut();

    match unsafe { namedpipeapi::CreatePipe(&mut read, &mut write, &mut SECURITY_ATTRIBUTES, 0) } {
        0 => Err(Error::OSError),
        _ => Ok((read, write)),
    }
}

pub struct ServiceListener {
    tcp_listener: TcpListener,
    command: String,
}

impl ServiceListener {
    pub fn new(port: u16, command: String) -> Result<Self> {
        let tcp_listener = TcpListener::bind(&SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port))?;

        Ok(Self {
            tcp_listener,
            command,
        })
    }

    pub fn get_tcp_listener(&self) -> &TcpListener {
        &self.tcp_listener
    }

    pub fn handle_connection(&self) -> Result<()> {
        let (stream, _) = self.tcp_listener.accept_std()?;

        info!(
            "Accepted new connection from {}, running: '{}'",
            stream.peer_addr()?,
            self.command
        );

        self.create_process(stream.into_raw_socket() as HANDLE)?;
        Ok(())
    }

    pub fn create_process(&self, client_socket: HANDLE) -> Result<(), Error> {
        let (stdin_read, stdin_write) = create_pipe()?;

        let mut startup_info = processthreadsapi::STARTUPINFOA {
            cb: mem::size_of::<processthreadsapi::STARTUPINFOA>() as u32,
            lpReserved: ptr::null_mut(),
            lpDesktop: ptr::null_mut(),
            lpTitle: ptr::null_mut(),
            dwX: 0,
            dwY: 0,
            dwXSize: 0,
            dwYSize: 0,
            dwXCountChars: 0,
            dwYCountChars: 0,
            dwFillAttribute: 0,
            dwFlags: winbase::STARTF_USESTDHANDLES,
            wShowWindow: 0,
            cbReserved2: 0,
            lpReserved2: ptr::null_mut(),
            hStdInput: stdin_read,
            hStdOutput: ptr::null_mut(),
            hStdError: ptr::null_mut(),
        };

        let mut process_info: processthreadsapi::PROCESS_INFORMATION = unsafe { mem::zeroed() };

        let result = unsafe {
            processthreadsapi::CreateProcessA(
                ptr::null_mut(),
                CString::new(self.command.clone()).unwrap().into_raw(),
                ptr::null_mut(),
                ptr::null_mut(),
                true as i32,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut startup_info,
                &mut process_info,
            )
        };

        if result == 0 {
            return Err(Error::OSError);
        }

        debug!("Created new proces, pid: {}", process_info.dwProcessId);

        let mut protocol_info: winsock2::WSAPROTOCOL_INFOW = unsafe { mem::zeroed() };

        let result = unsafe {
            winsock2::WSADuplicateSocketW(
                client_socket as winsock2::SOCKET,
                process_info.dwProcessId,
                &mut protocol_info,
            )
        };

        if result != 0 {
            return Err(Error::WSAError);
        }

        let result = unsafe { winsock2::closesocket(client_socket as winsock2::SOCKET) };

        if result != 0 {
            return Err(Error::WSAError);
        }

        let mut bytes_written: DWORD = 0;

        let write_result = unsafe {
            fileapi::WriteFile(
                stdin_write,
                &mut protocol_info as *mut _ as LPCVOID,
                mem::size_of::<winsock2::WSAPROTOCOL_INFOW>() as u32,
                &mut bytes_written,
                ptr::null_mut(),
            )
        };

        if write_result == 0 {
            return Err(Error::OSError);
        }

        assert!(bytes_written == mem::size_of::<winsock2::WSAPROTOCOL_INFOW>() as u32);

        unsafe {
            handleapi::CloseHandle(stdin_write);
            handleapi::CloseHandle(stdin_read);
        }
        Ok(())
    }
}
