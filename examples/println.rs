use tokio_serialport::SerialPort;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .expect("Please provide a path to a serial port");
    let _port = SerialPort::open(path, 115200).await?;
    Ok(())
}
