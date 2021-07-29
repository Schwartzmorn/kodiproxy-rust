use std::convert::TryInto;

use super::enums::*;
use super::functions::*;
use super::structs::*;

#[cfg_attr(test, mockall::automock)]
pub trait CECInterface: Sync + Send {
    /// Power on the given CEC devices. If [CECLogicalAddress::Broadcast] is given, then [LibcecConfiguration::wake_devices] is used
    fn power_on(&mut self, cec_logical_address: CECLogicalAddress) -> Result<(), CECError>;

    /// Put in standby mode the given CEC devices. If [CECLogicalAddress::Broadcast] is given, then [LibcecConfiguration::power_off_devices] is used
    fn standby(&mut self, cec_logical_address: CECLogicalAddress) -> Result<(), CECError>;
}

type LibcecConnectionT = *mut libc::c_void;

pub struct LibcecConfigurationBuilder {
    client_version: Result<u32, CECError>,
    callbacks: &'static mut ICECCallbacks,
}

pub struct CECConnection {
    connection: LibcecConnectionT,
    configuration: LibcecConfiguration,
}

unsafe impl Send for CECConnection {}
unsafe impl Sync for CECConnection {}

impl ICECCallbacks {
    extern "C" fn default_log_message(_cbparam: *mut libc::c_void, message: *const CECLogMessage) {
        if log::log_enabled!(log::Level::Debug) {
            unsafe {
                if let Some(msg) = message.as_ref().map(|m| m.message.as_ref()).flatten() {
                    let level = match message.as_ref().unwrap().level {
                        CECLogLevel::ERROR => log::Level::Warn,
                        CECLogLevel::WARNING => log::Level::Info,
                        _ => log::Level::Debug,
                    };
                    log::log!(
                        level,
                        "CEC log [{:?}]: {:?}",
                        message.as_ref().unwrap().level,
                        std::ffi::CStr::from_ptr(msg)
                    )
                }
            }
        }
    }
    extern "C" fn default_key_press(_cbparam: *mut libc::c_void, _key: *const CECKeypress) {}
    extern "C" fn default_command_received(
        _cbparam: *mut libc::c_void,
        _command: *const CECCommand,
    ) {
    }
    extern "C" fn default_configuration_changed(
        _cbparam: *mut libc::c_void,
        _configuration: *const LibcecConfiguration,
    ) {
    }
    extern "C" fn default_alert(
        _cbparam: *mut libc::c_void,
        alert: LibcecAlert,
        param: LibcecParameter,
    ) {
        if param.param_type == LibcecParameterType::String && !param.param_data.is_null() {
            unsafe {
                log::info!(
                    "CEC alert [{:?}]: {:?}",
                    alert,
                    std::ffi::CStr::from_ptr(
                        param.param_data.as_mut().unwrap() as *mut libc::c_void
                            as *mut std::os::raw::c_char
                    )
                );
            }
        } else {
            log::info!("CEC alert [{:?}]", alert);
        }
    }
    extern "C" fn default_menu_state_changed(
        _cbparam: *mut libc::c_void,
        _state: CECMenuState,
    ) -> libc::c_int {
        0
    }
    extern "C" fn default_source_activated(
        _cbparam: *mut libc::c_void,
        _logical_address: CECLogicalAddress,
        _b_activated: u8,
    ) {
    }
}

static mut ICECCALLBACKS_DEFAULT: ICECCallbacks = ICECCallbacks {
    log_message: ICECCallbacks::default_log_message,
    key_press: ICECCallbacks::default_key_press,
    command_received: ICECCallbacks::default_command_received,
    configuration_changed: ICECCallbacks::default_configuration_changed,
    alert: ICECCallbacks::default_alert,
    menu_state_changed: ICECCallbacks::default_menu_state_changed,
    source_activated: ICECCallbacks::default_source_activated,
};

impl LibcecConfigurationBuilder {
    pub fn new() -> Self {
        unsafe {
            LibcecConfigurationBuilder {
                client_version: Err(CECError::InvalidConfiguration(
                    "No version given for CEC client version",
                )),
                callbacks: &mut ICECCALLBACKS_DEFAULT,
            }
        }
    }

    pub fn with_client_version<T>(mut self, version: T) -> Self
    where
        T: std::convert::Into<String>,
    {
        self.client_version = LibcecConfigurationBuilder::parse_version(version.into());
        log::debug!("Using CEC version number {:?}", self.client_version);
        self
    }

    pub fn build(self) -> Result<LibcecConfiguration, CECError> {
        unsafe {
            let mut configuration = std::mem::zeroed::<LibcecConfiguration>();
            libcec_clear_configuration(&mut configuration);
            configuration.client_version = self.client_version?;
            configuration.device_types.types[0] = CECDeviceType::RecordingDevice;
            configuration.callbacks = self.callbacks;
            Ok(configuration)
        }
    }

    fn parse_version(version: String) -> Result<u32, CECError> {
        let mut versions: Vec<Result<u32, CECError>> = version
            .split('.')
            .map(|s| {
                s.parse()
                    .map_err(|_| CECError::InvalidConfiguration("Invalid CEC version"))
            })
            .take(3)
            .collect();
        versions.resize(3, Ok(0));
        let versions: [Result<u32, CECError>; 3] = versions.try_into().unwrap();
        Ok(versions[0]? << 16 | versions[1]? << 8 | versions[2]?)
    }
}

impl CECConnection {
    pub fn new(configuration: LibcecConfiguration) -> CECConnection {
        let mut connection = CECConnection {
            connection: std::ptr::null_mut(),
            configuration,
        };
        if let Err(e) = connection.reinit() {
            panic!("Failed to initialize the CEC connection: {:?}", e);
        }
        connection
    }

    fn reinit(&mut self) -> Result<(), CECError> {
        self.drop_connection();
        unsafe {
            self.connection = libcec_initialise(&mut self.configuration);
        }
        if self.connection.is_null() {
            return Err(CECError::InitFailed);
        }
        unsafe {
            libcec_init_video_standalone(self.connection);
        }
        let adapters = self.find_adapters()?;
        let adapter = adapters.first().ok_or(CECError::AdapterNotFound)?;
        log::info!("Connecting to CEC adapter {:?}", adapter);
        unsafe {
            if libcec_open(self.connection, adapter.comm.as_ptr(), 5000) == 0 {
                return Err(CECError::OpenFailed);
            }
            libcec_set_inactive_view(self.connection);
        }
        Ok(())
    }

    fn find_adapters(&mut self) -> Result<Vec<CECAdapter>, CECError> {
        let mut buf = [CECAdapter::default(); 10];
        let adapter_count = unsafe {
            libcec_find_adapters(
                self.connection,
                buf.as_mut_ptr(),
                buf.len() as u8,
                std::ptr::null_mut(),
            )
        };
        log::debug!("Found {} CEC adapters", adapter_count);
        if adapter_count >= 0 {
            Ok(buf
                .iter()
                .take(adapter_count as usize)
                .map(|x| *x)
                .collect())
        } else {
            Err(CECError::AdapterNotFound)
        }
    }

    // cleans the connection to the CEC adapter
    fn drop_connection(&mut self) {
        if !self.connection.is_null() {
            log::info!("Dropping connection to CEC adapters");
            unsafe {
                libcec_close(self.connection);
                libcec_destroy(self.connection);
            }
            self.connection = std::ptr::null_mut();
        }
    }

    fn exec<F>(&mut self, mut func: F) -> Result<(), CECError>
    where
        F: FnMut(&mut Self) -> libc::c_int,
    {
        if (&mut func)(self) == 0 {
            log::info!("Command failed, reinitializing connection");
            self.reinit()?;
            if (&mut func)(self) == 0 {
                log::info!("Command failed after reinitializing connection, not retrying");
                return Err(CECError::CommandFailed);
            }
        }
        Ok(())
    }
}

impl CECInterface for CECConnection {
    fn power_on(&mut self, cec_logical_address: CECLogicalAddress) -> Result<(), CECError> {
        self.exec(|s| unsafe { libcec_power_on_devices(s.connection, cec_logical_address) })
    }

    fn standby(&mut self, cec_logical_address: CECLogicalAddress) -> Result<(), CECError> {
        self.exec(|s| unsafe { libcec_standby_devices(s.connection, cec_logical_address) })
    }
}

impl Drop for CECConnection {
    fn drop(&mut self) {
        self.drop_connection();
    }
}
