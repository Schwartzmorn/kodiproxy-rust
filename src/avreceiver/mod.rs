pub use self::avreceiver::{AVReceiver, AVReceiverBuilder};

pub mod avreceiver;

cfg_if::cfg_if! {
    if #[cfg(test)] {
        pub use self::avreceiver::MockAVReceiver;
    }
}
