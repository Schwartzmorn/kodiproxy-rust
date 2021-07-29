#[derive(Clone, Copy, Debug)]
pub enum CECError {
    AdapterNotFound,
    CommandFailed,
    InitFailed,
    InvalidConfiguration(&'static str),
    OpenFailed,
}

// Contains the C bindings from https://github.com/Pulse-Eight/libcec
#[repr(C)]
#[allow(dead_code)]
#[derive(Debug)]
pub enum CECVersion {
    Unknown = 0x00,
    V1_2 = 0x01,
    V1_2A = 0x02,
    V1_3 = 0x03,
    V1_3A = 0x04,
    V1_4 = 0x05,
    V2_0 = 0x06,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug)]
pub enum CECDeviceType {
    TV = 0,
    RecordingDevice = 1,
    Reserved = 2,
    Tuner = 3,
    PlaybackDevice = 4,
    AudioSystem = 5,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug)]
pub enum CECMenuState {
    Activated = 0,
    Deactivated = 1,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug)]
pub enum CECPowerStatus {
    On = 0x00,
    Standby = 0x01,
    InTransitionStandbyToOn = 0x02,
    InTransitionOnToStandby = 0x03,
    Unknown = 0x99,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug)]
pub enum CECUserControlCode {
    Select = 0x00,
    // Add other codes ?
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub enum CECLogicalAddress {
    Unknown = -1,
    TV = 0,
    RecordingDevice1 = 1,
    RecordingDevice2 = 2,
    Tuner1 = 3,
    PlaybackDevice1 = 4,
    AudioSystem = 5,
    Tuner2 = 6,
    Tuner3 = 7,
    PlaybackDevice2 = 8,
    RecordingDevice3 = 9,
    Tuner4 = 10,
    PlaybackDevice3 = 11,
    Reserved1 = 12,
    Reserved2 = 13,
    FreeUse = 14,
    Broadcast = 15,
    // Unregistered = 15,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug)]
pub enum CECOpcode {
    ActivateSource = 0x82,
    ImageViewOn = 0x04,
    TextViewOn = 0x0D,
    IncativeSource = 0x9D,
    RequestActiveSource = 0x85,
    RoutingChange = 0x80,
    RoutingInformation = 0x81,
    SetStreamPath = 0x86,
    Standby = 0x36,
    RecordOff = 0x0B,
    RecordOn = 0x09,
    RecordStatus = 0x0A,
    RecordTVScreen = 0x0F,
    ClearAnalogueTimer = 0x33,
    CleatDigitalTimer = 0x99,
    ClearExternalTimer = 0xA1,
    SetAnalogueTimer = 0x34,
    SetDigitalTimer = 0x97,
    SetExternalTimer = 0xA2,
    SetTimerProgramTitle = 0x67,
    TimerClearedStatus = 0x43,
    TimerStatus = 0x35,
    CECVersion = 0x9E,
    GetCECVersion = 0x9F,
    GivePhysicalAddress = 0x83,
    GetMenuLanguage = 0x91,
    ReportPhysicalAddress = 0x84,
    SetMenuLanguage = 0x32,
    DeckControl = 0x42,
    DeckStatus = 0x1B,
    GiveDeckStatus = 0x1A,
    Play = 0x41,
    GiveTunerDeviceStatus = 0x08,
    SelectAnalogueService = 0x92,
    SelectDigitalService = 0x93,
    TunerDeviceStatus = 0x07,
    TunerStepDecrement = 0x06,
    TunerStepIncrement = 0x05,
    DeviceVendorId = 0x87,
    GiveDeviceVendorId = 0x8C,
    VendorCommand = 0x89,
    VendorCommandWithId = 0xA0,
    VendorRemoteButtonDown = 0x8A,
    VendorRemoteButtonUp = 0x8B,
    SetOsdString = 0x64,
    GiveOsdName = 0x46,
    SetOsdName = 0x47,
    MenuRequest = 0x8D,
    MenuStatus = 0x8E,
    UserControlPressed = 0x44,
    UserControlRelease = 0x45,
    GiveDevicePowerStatus = 0x8F,
    ReportPowerStatus = 0x90,
    FeatureAbort = 0x00,
    Abort = 0xFF,
    GiveAudioStatus = 0x71,
    GiveSystemAudioModeStatus = 0x7D,
    ReportAudioStatus = 0x7A,
    SetSystemAudioMode = 0x72,
    SystemAudioModeRequest = 0x70,
    SystemAudioModeStatus = 0x7E,
    SetAudioRate = 0x9A,
    // CEC 1.4
    ReportShortAudioDescriptors = 0xA3,
    RequestShortAudioDescriptors = 0xA4,
    StartArc = 0xC0,
    ReportArcStarted = 0xC1,
    ReportArcEnded = 0xC2,
    RequestArcStart = 0xC3,
    RequestArcEnd = 0xC4,
    EndArc = 0xC5,
    Cdc = 0xF8,
    // when this opcode is set, no opcode will be sent to the device. this is one of the reserved numbers
    None = 0xFD,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug)]
pub enum CECLogLevel {
    ERROR = 1,
    WARNING = 2,
    NOTICE = 4,
    TRAFFIC = 8,
    DEBUG = 16,
    ALL = 31,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug)]
pub enum CECAdapterType {
    Unknown = 0,
    P8External = 0x1,
    P8Daughterboard = 0x2,
    RPI = 0x100,
    TDA995x = 0x200,
    Exynos = 0x300,
    AOCEC = 0x500,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug)]
pub enum LibcecAlert {
    ServiceDevice,
    ConnectionLost,
    PermissionError,
    PortBusy,
    PhysicalAddressError,
    TVPollFailed,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum LibcecParameterType {
    String,
    Unknown,
}

impl std::str::FromStr for CECLogicalAddress {
    type Err = CECError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "TV" => Ok(CECLogicalAddress::TV),
            "RECORDING_DEVICE_1" => Ok(CECLogicalAddress::RecordingDevice1),
            "RECORDING_DEVICE_2" => Ok(CECLogicalAddress::RecordingDevice2),
            "TUNER_1" => Ok(CECLogicalAddress::Tuner1),
            "PLAYBACK_DEVICE_1" => Ok(CECLogicalAddress::PlaybackDevice1),
            "AUDIO_SYSTEM" => Ok(CECLogicalAddress::AudioSystem),
            "TUNER_2" => Ok(CECLogicalAddress::Tuner2),
            "TUNER_3" => Ok(CECLogicalAddress::Tuner3),
            "PLAYBACK_DEVICE_2" => Ok(CECLogicalAddress::PlaybackDevice2),
            "RECORDING_DEVICE_3" => Ok(CECLogicalAddress::RecordingDevice3),
            "TUNER_4" => Ok(CECLogicalAddress::Tuner4),
            "PLAYBACK_DEVICE_3" => Ok(CECLogicalAddress::PlaybackDevice3),
            "RESERVED_1" => Ok(CECLogicalAddress::Reserved1),
            "RESERVED_2" => Ok(CECLogicalAddress::Reserved2),
            "FREE_USE" => Ok(CECLogicalAddress::FreeUse),
            "BROADCAST" => Ok(CECLogicalAddress::Broadcast),
            _ => Err(CECError::InvalidConfiguration("Invalid logical address")),
        }
    }
}
