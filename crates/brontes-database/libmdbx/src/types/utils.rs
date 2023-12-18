pub(crate) mod address_string {
    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &Address, serializer: S) -> Result<S::Ok, S::Error> {
        format!("{:?}", u).serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Address, D::Error>
    where
        D: Deserializer<'de>,
    {
        let address: String = Deserialize::deserialize(deserializer)?;

        Ok(Address::from_str(&address).map_err(serde::de::Error::custom)?)
    }
}

pub(crate) mod pool_tokens {
    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::types::address_to_tokens::PoolTokens;

    pub fn serialize<S: Serializer>(u: &PoolTokens, serializer: S) -> Result<S::Ok, S::Error> {
        u.clone()
            .into_iter()
            .map(|a| format!("{:?}", a))
            .collect::<Vec<String>>()
            .serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<PoolTokens, D::Error>
    where
        D: Deserializer<'de>,
    {
        let addresses: Vec<String> = Deserialize::deserialize(deserializer)?;

        Ok(addresses.into())
    }
}

pub(crate) mod static_bindings {

    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    use crate::types::address_to_protocol::StaticBindingsDb;

    pub fn serialize<S: Serializer>(
        u: &StaticBindingsDb,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let st: String = u.clone().into();
        st.serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<StaticBindingsDb, D::Error>
    where
        D: Deserializer<'de>,
    {
        let address: Option<String> = Deserialize::deserialize(deserializer)?;

        Ok(address.unwrap().into())
    }
}

pub(crate) mod u256 {

    use std::str::FromStr;

    use alloy_primitives::U256;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &U256, serializer: S) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: String = Deserialize::deserialize(deserializer)?;

        Ok(U256::from_str(&data).map_err(serde::de::Error::custom)?)
    }
}

pub(crate) mod address {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &Address, serializer: S) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Address, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: String = Deserialize::deserialize(deserializer)?;

        Ok(Address::from_str(&data).map_err(serde::de::Error::custom)?)
    }
}

pub(crate) mod vec_txhash {

    use std::str::FromStr;

    use alloy_primitives::{Address, TxHash};
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &Vec<TxHash>, serializer: S) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<TxHash>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: Vec<String> = Deserialize::deserialize(deserializer)?;

        Ok(data
            .into_iter()
            .map(|d| TxHash::from_str(&d))
            .collect::<Result<Vec<_>, <TxHash as FromStr>::Err>>()
            .map_err(serde::de::Error::custom)?)
    }
}

pub(crate) mod option_address {

    use std::str::FromStr;

    use alloy_primitives::Address;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Serialize, Serializer},
    };

    pub fn serialize<S: Serializer>(u: &Option<Address>, serializer: S) -> Result<S::Ok, S::Error> {
        let st: String = format!("{:?}", u.clone());
        st.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Address>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let des: Option<String> = Deserialize::deserialize(deserializer)?;
        let data = des.map(|d| Address::from_str(&d));

        Ok(data
            .map_or_else(|| Ok(None), |res| res.map(Some))
            .map_err(serde::de::Error::custom)?)
    }
}
