// Contains the C bindings from https://github.com/Pulse-Eight/libcec
use crate::cec::enums::*;

const CEC_MAX_DATA_PACKET_SIZE: usize = 16 * 4;

#[repr(C)]
#[derive(Debug)]
pub struct CECLogMessage {
    pub message: *const libc::c_char,
    pub level: CECLogLevel,
    pub time: i64,
}

#[repr(C)]
#[derive(Debug)]
pub struct CECKeypress {
    pub keycode: CECUserControlCode,
    pub duration: libc::c_int,
}

#[repr(C)]
#[derive(Copy)]
pub struct CECAdapter {
    pub path: [libc::c_char; 1024],
    pub comm: [libc::c_char; 1024],
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug)]
pub struct CECDatapacket {
    pub data: [u8; CEC_MAX_DATA_PACKET_SIZE],
    pub size: u8,
}

#[repr(C)]
#[derive(Debug)]
pub struct CECCommand {
    pub initiator: CECLogicalAddress,
    pub destination: CECLogicalAddress,
    pub ack: i8,
    pub eom: i8,
    pub opcode: CECOpcode,
    pub parameters: CECDatapacket,
    pub opcode_set: i8,
    pub transmit_timeout: i32,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug)]
pub struct CECDeviceTypeList {
    pub types: [CECDeviceType; 5],
}

#[repr(C)]
#[derive(Debug)]
pub struct CECLogicalAddresses {
    pub primary: CECLogicalAddress,
    pub addresses: [libc::c_int; 16],
}

#[repr(C)]
#[derive(Debug)]
pub struct LibcecParameter {
    pub param_type: LibcecParameterType,
    pub param_data: *mut libc::c_void,
}

#[repr(C)]
pub struct ICECCallbacks {
    pub log_message: extern "C" fn(*mut libc::c_void, *const CECLogMessage),
    pub key_press: extern "C" fn(*mut libc::c_void, *const CECKeypress),
    pub command_received: extern "C" fn(*mut libc::c_void, *const CECCommand),
    pub configuration_changed: extern "C" fn(*mut libc::c_void, *const LibcecConfiguration),
    pub alert: extern "C" fn(*mut libc::c_void, LibcecAlert, LibcecParameter),
    pub menu_state_changed: extern "C" fn(*mut libc::c_void, CECMenuState) -> libc::c_int,
    pub source_activated: extern "C" fn(*mut libc::c_void, CECLogicalAddress, u8),
}

#[repr(C)]
#[allow(dead_code)]
pub struct LibcecConfiguration {
    pub client_version: u32, // the version of the client that is connecting
    pub str_device_name: [libc::c_char; 13], // the device name to use on the CEC bus, name + 0 terminator
    pub device_types: CECDeviceTypeList,     // the device type(s) to use on the CEC bus for libCEC
    pub b_autodetect_address: u8, // (read only) set to 1 by libCEC when the physical address was autodetected
    pub i_physical_address: u16,  // the physical address of the CEC adapter
    pub base_device: CECLogicalAddress, //  the logical address of the device to which the adapter is connected. only used when iPhysicalAddress = 0 or when the adapter doesn't support autodetection
    pub i_hdmi_port: u8, // the HDMI port to which the adapter is connected. only used when iPhysicalAddress = 0 or when the adapter doesn't support autodetection
    pub tv_vendor: u32,  // override the vendor ID of the TV. leave this untouched to autodetect
    pub wake_devices: CECLogicalAddresses, // list of devices to wake when initialising libCEC or when calling PowerOnDevices() without any parameter.
    pub power_off_devices: CECLogicalAddresses, // list of devices to power off when calling StandbyDevices() without any parameter.
    pub server_version: u32,                    // the version number of the server. read-only
    // player specific settings
    pub b_get_settings_from_rom: u8, // true to get the settings from the ROM (if set, and a v2 ROM is present), false to use these settings.
    pub b_activate_source: u8, // make libCEC the active source on the bus when starting the player application
    pub b_power_off_on_standby: u8, // put this PC in standby mode when the TV is switched off. only used when bShutdownOnStandby = 0

    pub callback_param: *mut libc::c_void, // the object to pass along with a call of the callback methods. NULL to ignore
    pub callbacks: *mut ICECCallbacks, // the callback methods to use. set this to NULL when not using callbacks

    pub logical_addresses: CECLogicalAddresses, // (read-only) the current logical addresses. added in 1.5.3
    pub i_firmware_version: u16, // (read-only) the firmware version of the adapter. added in 1.6.0
    pub str_device_language: [libc::c_char; 3], // the menu language used by the client. 3 character ISO 639-2 country code. see http://http://www.loc.gov/standards/iso639-2/ added in 1.6.2
    pub i_firmware_build_date: u32, // (read-only) the build date of the firmware, in seconds since epoch. if not available, this value will be set to 0. added in 1.6.2
    pub b_monitor_only: u8, // won't allocate a CCECClient when starting the connection when set (same as monitor mode). added in 1.6.3
    pub cec_version: CECVersion, // CEC spec version to use by libCEC. defaults to v1.4. added in 1.8.0
    pub adapter_type: CECAdapterType, // type of the CEC adapter that we're connected to. added in 1.8.2
    pub combo_key: CECUserControlCode, // key code that initiates combo keys. defaults to CEC_USER_CONTROL_CODE_STOP. CEC_USER_CONTROL_CODE_UNKNOWN to disable. added in 2.0.5
    pub i_combo_key_timeout_ms: u32,   // timeout until the combo key is sent as normal keypres
    pub i_button_repeat_rate_ms: u32, // rate at which buttons autorepeat. 0 means rely on CEC device
    pub i_button_release_delay_ms: u32, // duration after last update until a button is considered released
    pub i_double_tap_timeout_ms: u32, // prevent double taps within this timeout. defaults to 200ms. added in 4.0.0
    pub b_auto_wake_avr: u8, // set to 1 to automatically waking an AVR when the source is activated. added in 4.0.0

                             // for cec version >= 5
                             // pub b_auto_power_on: u8,
}

impl Default for CECAdapter {
    fn default() -> CECAdapter {
        CECAdapter {
            path: [0; 1024],
            comm: [0; 1024],
        }
    }
}

impl Clone for CECAdapter {
    fn clone(&self) -> CECAdapter {
        CECAdapter {
            path: self.path,
            comm: self.comm,
        }
    }
}

impl std::fmt::Debug for CECAdapter {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        unsafe {
            fmt.debug_struct("CECAdapter")
                .field("path", &std::ffi::CStr::from_ptr(self.path.as_ptr()))
                .field("comm", &std::ffi::CStr::from_ptr(self.comm.as_ptr()))
                .finish()
        }
    }
}
