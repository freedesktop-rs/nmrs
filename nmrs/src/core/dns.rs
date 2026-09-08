//! Global DNS configuration property reads and writes.

use zbus::Connection;

use crate::Result;
use crate::api::models::{ConnectionError, GlobalDnsConfiguration};
use crate::dbus::NMProxy;

pub(crate) async fn global_dns_configuration(conn: &Connection) -> Result<GlobalDnsConfiguration> {
    let nm = NMProxy::new(conn).await?;
    let raw =
        nm.global_dns_configuration()
            .await
            .map_err(|source| ConnectionError::DbusOperation {
                context: "read GlobalDnsConfiguration property".into(),
                source,
            })?;
    Ok(GlobalDnsConfiguration::from_dbus(&raw))
}

pub(crate) async fn set_global_dns_configuration(
    conn: &Connection,
    config: &GlobalDnsConfiguration,
) -> Result<()> {
    config.validate()?;

    let nm = NMProxy::new(conn).await?;
    nm.set_global_dns_configuration(config.to_dbus())
        .await
        .map_err(|source| ConnectionError::DbusOperation {
            context: "set GlobalDnsConfiguration property".into(),
            source,
        })
}
