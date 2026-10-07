use zbus::Connection;

pub trait ActiveTransport {
    type Output;

    async fn current(conn: &Connection) -> Option<Self::Output>;
}
