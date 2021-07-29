// Contains the C bindings from https://github.com/Pulse-Eight/libcec
use crate::cec::enums::*;
use crate::cec::structs::*;

type LibcecConnectionT = *mut libc::c_void;

#[link(name = "cec")]
extern "C" {
    pub fn libcec_destroy(connection: LibcecConnectionT);
    pub fn libcec_initialise(configuration: *mut LibcecConfiguration) -> LibcecConnectionT;
    pub fn libcec_open(
        connection: LibcecConnectionT,
        str_port: *const libc::c_char,
        i_timeout: u32,
    ) -> libc::c_int;
    pub fn libcec_close(connection: LibcecConnectionT);
    pub fn libcec_init_video_standalone(connection: LibcecConnectionT);
    pub fn libcec_find_adapters(
        connection: LibcecConnectionT,
        device_list: *mut CECAdapter,
        i_buf_size: u8,
        str_device_path: *mut libc::c_char,
    ) -> i8;
    pub fn libcec_power_on_devices(
        connection: LibcecConnectionT,
        cec_logical_address: CECLogicalAddress,
    ) -> libc::c_int;
    pub fn libcec_standby_devices(
        connection: LibcecConnectionT,
        cec_logical_address: CECLogicalAddress,
    ) -> libc::c_int;
    /// Broadcast a message that notifies connected CEC capable devices that this device is no longer the active source.
    pub fn libcec_set_inactive_view(connection: LibcecConnectionT) -> libc::c_int;
    pub fn libcec_clear_configuration(configuration: *mut LibcecConfiguration);
}
