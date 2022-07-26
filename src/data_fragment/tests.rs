// #[cfg(test)]
// mod data_fragment_tests {
//     use cid::{
//         multihash::{Code, MultihashDigest},
//         Cid,
//     };
//
//     use crate::data_fragment::traits::FragmentAccessor;
//     use crate::data_fragment::{self, traits::LiveFragment, DataFragment};
//
//     #[test]
//     fn expect_default() {
//         let mut fragment: DataFragment = DataFragment::default();
//         assert_eq!(fragment.v, -1);
//         fragment.set(Some(String::from("Inital data")));
//         assert_eq!(fragment.v, 0);
//     }
//
//     #[test]
//     fn expect_cid_matching() {
//         let fragment: DataFragment = DataFragment::from(String::from("Inital data"));
//         let h = Code::Sha2_256.digest("MockData".as_bytes());
//         let cid = Cid::new_v1(data_fragment::RAW, h);
//         assert_eq!(fragment.cid, cid);
//     }
//
//     #[test]
//     fn expect_cid_remains_unchanged() {
//         let mut fragment: DataFragment = DataFragment::from(String::from("Inital data"));
//         let cid: Cid = fragment.cid;
//         fragment.set(Some(String::from("Hello, world!")));
//         assert_eq!(cid, fragment.cid);
//     }
//
//     #[test]
//     fn expect_timestamp_incrementing() {
//         let mut fragment: DataFragment = DataFragment::from(String::from("Inital data"));
//         let timestamp: i64 = fragment.timestamp;
//         fragment.set(Some(String::from("Hello, world!")));
//         assert_ne!(timestamp, fragment.timestamp);
//     }
//
//     #[test]
//     fn expect_static_data_dead() {
//         let fragment: DataFragment = DataFragment::from(String::from("Inital data"));
//         assert_eq!(fragment.alive, false);
//     }
//
//     #[test]
//     fn expect_stream_alive() {
//         let mut fragment: DataFragment = DataFragment::from(String::from("Inital data"));
//         fragment.wake();
//         assert_eq!(fragment.alive, true);
//     }
// }
