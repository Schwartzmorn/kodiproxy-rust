static EMPTY_ARRAY: [&[u8]; 0] = [];

static AVAHI_BUS: &'static str = "org.freedesktop.Avahi";
static AVAHI_ENTRY_INTERFACE: &'static str = "org.freedesktop.Avahi.EntryGroup";

pub struct AvahiConnection<'a> {
    dbus_connection: dbus::blocking::Connection,
    dbus_path: dbus::Path<'a>,
}

impl<'a> AvahiConnection<'a> {
    pub fn new(port: u16) -> Result<AvahiConnection<'a>, dbus::Error> {
        log::info!("Opening connection with Dbus");
        let dbus_connection = dbus::blocking::Connection::new_system()?;

        let (dbus_path,): (dbus::Path,) = dbus_connection
            .with_proxy(AVAHI_BUS, "/", std::time::Duration::from_millis(2000))
            .method_call("org.freedesktop.Avahi.Server", "EntryGroupNew", ())?;

        log::debug!("Got path: {:?}", dbus_path);

        let dbus_proxy = dbus_connection.with_proxy(
            AVAHI_BUS,
            dbus_path.clone(),
            std::time::Duration::from_millis(2000),
        );

        dbus_proxy.method_call(
            AVAHI_ENTRY_INTERFACE,
            "AddService",
            (
                -1i32,                  // interface index => -1 means unspecified
                -1i32,                  // protocol => -1 means unspecified, 0 means ipv4
                0u32,                   // flags
                "Kodiproxy (rust)",     // name of the entry
                "_xbmc-jsonrpc-h._tcp", // type of the entry
                "",                     // domain
                "",                     // host
                port,                   // port
                EMPTY_ARRAY.as_ref(),   // text: array of array of bytes...
            ),
        )?;

        dbus_proxy.method_call(AVAHI_ENTRY_INTERFACE, "Commit", ())?;

        log::info!("Registered server in Avahi");

        Ok(AvahiConnection {
            dbus_connection,
            dbus_path,
        })
    }
}

impl<'a> Drop for AvahiConnection<'a> {
    fn drop(&mut self) {
        log::info!("Freeing Avahi entry");
        let res: Result<(), dbus::Error> = self
            .dbus_connection
            .with_proxy(
                AVAHI_BUS,
                self.dbus_path.clone(),
                std::time::Duration::from_millis(2000),
            )
            .method_call(AVAHI_ENTRY_INTERFACE, "Free", ());
        if let Err(e) = res {
            log::warn!("Failed to call Free: {:?}", e);
        }
    }
}
