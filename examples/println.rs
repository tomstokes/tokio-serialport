use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .expect("Please provide a path to a serial port");
    let mut port = SerialPort::builder(path, 115200)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .open()
        .await?;
    let mut bytes = [0; 32];
    let result = port.write("hello world".as_bytes()).await;
    let _ = dbg!(result);
    let result = port.read(&mut bytes).await;
    let _ = dbg!(result);
    let _ = dbg!(str::from_utf8(&bytes));
    Ok(())
}
