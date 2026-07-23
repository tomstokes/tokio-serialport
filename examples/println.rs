use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serialport::SerialPort;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .expect("Please provide a path to a serial port");
    let mut port = SerialPort::open(path, 115200).await?;
    let mut bytes = [0; 32];
    let result = port.write("hello world".as_bytes()).await;
    let _ = dbg!(result);
    let result = port.read(&mut bytes).await;
    let _ = dbg!(result);
    let _ = dbg!(str::from_utf8(&bytes));
    Ok(())
}
