use anyhow::Result;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{from_slice as json_from_slice, to_vec as json_to_vec};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, Stdin, Stdout},
    process::{ChildStdin, ChildStdout},
};

pub trait AsyncSink {
    async fn read<D: DeserializeOwned>(&mut self) -> Result<Option<D>>;
    async fn write<S: Serialize>(&mut self, value: &S) -> Result<()>;
}

pub struct Child {
    stdin: ChildStdin,
    stdout: ChildStdout,
    buffer: [u8; 4096],
}

impl AsyncSink for Child {
    async fn read<D: DeserializeOwned>(&mut self) -> Result<Option<D>> {
        let size = self.stdout.read(&mut self.buffer[..]).await?;
        Ok(json_from_slice(&self.buffer[..size]).ok())
    }

    async fn write<S: Serialize>(&mut self, value: &S) -> Result<()> {
        self.stdin.write_all(&json_to_vec(value)?).await?;
        Ok(())
    }
}

pub struct Current {
    stdin: Stdin,
    stdout: Stdout,
    buffer: [u8; 4096],
}

impl AsyncSink for Current {
    async fn read<D: DeserializeOwned>(&mut self) -> Result<Option<D>> {
        let size = self.stdin.read(&mut self.buffer[..]).await?;
        Ok(json_from_slice(&self.buffer[..size]).ok())
    }

    async fn write<S: Serialize>(&mut self, value: &S) -> Result<()> {
        self.stdout.write_all(&json_to_vec(value)?).await?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
enum Kind {
    Request,
    Response,
}

#[derive(Debug, Deserialize, Serialize)]
struct Payload<T> {
    id: u32,
    kind: Kind,
    payload: T,
}

pub struct Interprocess<T: Sink> {
    sink: T,
}

impl<T: Sink> Interprocess<T> {
    pub fn call<R: Serialize, S: DeserializeOwned>(&mut self, args: R) -> Result<S> {}
}
