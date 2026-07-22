use tokio_serialport::SerialPort;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let _port = SerialPort::open("/dev/ttyUSB0", 115200).await?;
    Ok(())
}
