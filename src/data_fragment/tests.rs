
#[cfg(test)]
mod data_fragment_tests {
    use cid::{Cid, multihash::{Code, MultihashDigest}};

    use crate::data_fragment::{DataFragment, StaticFragment, self, LiveFragment};

    #[test]
    fn it_works() {
        let mut fragment: DataFragment = DataFragment::default().from("MockData".to_string());
        assert_eq!(fragment.v, 0);
        fragment.update("Hello, World!".to_string());
        assert_eq!(fragment.v, 1);
    }

    #[test]
    fn expect_cid_matching() {
        let mut fragment: DataFragment = DataFragment::default().from("MockData".to_string());
        let h = Code::Sha2_256.digest("MockData".as_bytes());
        let cid = Cid::new_v1(data_fragment::RAW, h);
        assert_eq!(fragment.cid, cid);
    }

    #[test]
    fn expect_cid_remains_unchanged() {
        let mut fragment: DataFragment = DataFragment::default().from("MockData".to_string());
        let cid: Cid = fragment.cid;
        fragment.update("Hello, World!".to_string());
        assert_eq!(cid, fragment.cid);
    }

    #[test]
    fn expect_timestamp_incrementing() {
        let mut fragment: DataFragment = DataFragment::default().from("MockData".to_string());
        let timestamp: i64 = fragment.timestamp;
        fragment.update("Hello, World!".to_string());
        assert_ne!(timestamp, fragment.timestamp);
    }

    #[test]
    fn expect_static_data_dead() {
        let fragment: DataFragment = DataFragment::default().from("MockData".to_string());
        assert_eq!(fragment.alive, false);
    }

    #[test]
    fn expect_stream_alive() {
        let mut fragment: DataFragment = DataFragment::default().from("MockData".to_string());
        fragment.live();
        assert_eq!(fragment.alive, true);
    }
}