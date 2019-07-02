# Winetd
A partial implementation of [inetd](https://en.wikipedia.org/wiki/Inetd) functionality in
Windows. `Winetd` runs as a service, listens to incoming TCP connections and runs a configured command. The client socket is passed to the executable, allowing them to comminucate.

## Usage

 1. Install using `msiexec`: `msiexec /i winetd.msi /l*v install.log /q`. `Winetd` service will start immediately, and run automatically on system boot.
 2. Each service is defined by creating a TOML file in `C:\ProgramData\Winetd`, for example:
```toml
 port = 1234
 command = "C:\\Program Files\\MyProgram\\main.exe"
 ```

 3. Restart the service: `Restart-Service winetd` (Powershell) or `sc.exe stop winetd; sc.exe start
    winetd` (cmd).
 4. That's it, the listener is ready. In case anything goes wrong, logs are in Event Viewer.

**Important Notice**

Unlike UNIX, one
[cannot](https://stackoverflow.com/questions/4993119/redirect-io-of-process-to-windows-socket)
simply pass socket handle as standard input/output to a newly created process.
The method we chose is to write the incoming socket (`WSA_PROTOCOLINFOW` struct) to the child process `STDIN`, then the child process has to recreate the socket (using `WSASocketW` function).
Therefore `Winetd` **can't be used directly on any arbitrary executable**. In order to use it, a wrapper which would extract the socket from `STDIN` must be added. Sample code that does the job in C++:

```c++
    #include <winsock2.h>

    HANDLE getHandle()
    {
        WSAData wsa_data;
        DWORD bytes_read = 0;
        WSAPROTOCOL_INFOW protocol_info;

        if (const auto result = WSAStartup(MAKEWORD(2, 2), &wsa_data); result != 0) {
            std::abort();
        }

        if (const auto result = ReadFile(GetStdHandle(STD_INPUT_HANDLE), (LPWSAPROTOCOL_INFOW) &protocol_info,
                                         sizeof(WSAPROTOCOL_INFOW), &bytes_read, nullptr);
            !result) {
            std::abort();
        }

        const SOCKET socket =
            WSASocketW(FROM_PROTOCOL_INFO, FROM_PROTOCOL_INFO, FROM_PROTOCOL_INFO, &protocol_info, 0, 0);
        if (socket == INVALID_SOCKET) {
            std::abort();
        }

        return reinterpret_cast<HANDLE>(socket);
    }
```

Once the socket is created, it can be used to communicate with the client almost interchangeably with any other handle (i.e `ReadFile`, `WriteFile`, etc).
