// battleship/network.rs — TCP networking layer for Battleship.
// Handles hosting (bind + accept) and joining (connect), plus
// sending and receiving newline-delimited JSON messages over the stream.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use super::protocol::Message;

// ---------------------------------------------------------------------------
// Connection — wraps a TcpStream with a buffered reader for line-based I/O
// ---------------------------------------------------------------------------

/// A live connection to the remote peer.
/// Bundles the raw `TcpStream` (for writing) with a `BufReader` (for
/// line-based reading) so callers don't have to manage both separately.
pub struct Connection {
    /// Buffered reader over the incoming half of the TCP stream.
    reader: BufReader<TcpStream>,
    /// The raw stream clone used for writing outgoing messages.
    writer: TcpStream,
}

impl Connection {
    /// Wrap an established `TcpStream` into a `Connection`.
    /// Clones the stream internally so reads and writes are independent.
    fn from_stream(stream: TcpStream) -> io::Result<Self> {
        let writer = stream.try_clone()?;
        let reader = BufReader::new(stream);
        Ok(Connection { reader, writer })
    }

    // -- Sending messages ---------------------------------------------------

    /// Serialize `msg` as JSON and write it as a single line to the stream.
    /// Flushes immediately so the peer receives it without delay.
    pub fn send(&mut self, msg: &Message) -> io::Result<()> {
        let json = serde_json::to_string(msg)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        writeln!(self.writer, "{}", json)?;
        self.writer.flush()?;
        Ok(())
    }

    // -- Receiving messages (blocking) --------------------------------------

    /// Block until a full line arrives, then deserialize it as a `Message`.
    /// Returns `Ok(None)` if the peer closed the connection (EOF).
    /// Returns `Err` on I/O or parse errors.
    pub fn recv(&mut self) -> io::Result<Option<Message>> {
        let mut line = String::new();
        let bytes_read = self.reader.read_line(&mut line)?;

        // EOF — peer disconnected
        if bytes_read == 0 {
            return Ok(None);
        }

        let msg: Message = serde_json::from_str(line.trim())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Some(msg))
    }

    // -- Non-blocking receive -----------------------------------------------

    /// Try to read a message without blocking.
    /// Returns `Ok(Some(msg))` if a complete line was available,
    /// `Ok(None)` if no data is ready yet or the peer disconnected,
    /// and `Err` on real I/O failures.
    ///
    /// Callers should call `set_nonblocking(true)` on the underlying stream
    /// before using this in a game loop.
    pub fn try_recv(&mut self) -> io::Result<Option<Message>> {
        // Temporarily enable non-blocking mode on the reader's stream
        self.reader.get_ref().set_nonblocking(true)?;

        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => {
                // EOF — peer disconnected
                self.reader.get_ref().set_nonblocking(false)?;
                Ok(None)
            }
            Ok(_) => {
                self.reader.get_ref().set_nonblocking(false)?;
                let msg: Message = serde_json::from_str(line.trim())
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                Ok(Some(msg))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No data available right now — that's fine
                self.reader.get_ref().set_nonblocking(false)?;
                Ok(None)
            }
            Err(e) => {
                self.reader.get_ref().set_nonblocking(false)?;
                Err(e)
            }
        }
    }

    // -- Handshake ----------------------------------------------------------

    /// Perform the Hello handshake: send our name, receive the opponent's.
    /// Returns the opponent's display name on success.
    pub fn handshake(&mut self, my_name: &str) -> io::Result<String> {
        // Send our Hello
        self.send(&Message::Hello {
            name: my_name.to_string(),
        })?;

        // Wait for their Hello
        match self.recv()? {
            Some(Message::Hello { name }) => Ok(name),
            Some(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Expected Hello message during handshake",
            )),
            None => Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Peer disconnected during handshake",
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Host — bind to a port and wait for a client to connect
// ---------------------------------------------------------------------------

/// Start hosting a game on the given port.
/// Binds a TCP listener, waits for exactly one client to connect,
/// and returns a `Connection` ready for the handshake.
pub fn host(port: u16) -> io::Result<Connection> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)?;
    println!("Listening on {} — waiting for opponent...", addr);

    // Accept the first incoming connection
    let (stream, peer_addr) = listener.accept()?;
    println!("Opponent connected from {}", peer_addr);

    Connection::from_stream(stream)
}

/// Connect to a host at the given address (e.g. "192.168.1.5:7878").
/// Returns a `Connection` ready for the handshake.
pub fn connect(addr: &str) -> io::Result<Connection> {
    println!("Connecting to {}...", addr);
    let stream = TcpStream::connect(addr)?;
    println!("Connected!");
    Connection::from_stream(stream)
}

// ===========================================================================
// Unit tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// Spin up a host and client on localhost, exchange Hello messages,
    /// then verify the handshake worked for both sides.
    #[test]
    fn host_client_handshake() {
        // Use port 0 so the OS picks a free port (avoids conflicts)
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        // Host thread: accept one connection and perform handshake
        let host_handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut conn = Connection::from_stream(stream).unwrap();
            let opponent = conn.handshake("HostPlayer").unwrap();
            opponent
        });

        // Client: connect and perform handshake
        let addr = format!("127.0.0.1:{}", port);
        let stream = TcpStream::connect(&addr).unwrap();
        let mut client_conn = Connection::from_stream(stream).unwrap();
        let opponent = client_conn.handshake("ClientPlayer").unwrap();

        // Client should see the host's name
        assert_eq!(opponent, "HostPlayer");

        // Host should see the client's name
        let host_opponent = host_handle.join().unwrap();
        assert_eq!(host_opponent, "ClientPlayer");
    }

    /// Test sending and receiving individual messages over a connection.
    #[test]
    fn send_recv_messages() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let sender_handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut conn = Connection::from_stream(stream).unwrap();

            // Send a sequence of messages
            conn.send(&Message::Ready).unwrap();
            conn.send(&Message::Fire { x: 3, y: 7 }).unwrap();
            conn.send(&Message::Disconnect).unwrap();
        });

        let addr = format!("127.0.0.1:{}", port);
        let stream = TcpStream::connect(&addr).unwrap();
        let mut conn = Connection::from_stream(stream).unwrap();

        // Receive and verify each message in order
        assert_eq!(conn.recv().unwrap(), Some(Message::Ready));
        assert_eq!(conn.recv().unwrap(), Some(Message::Fire { x: 3, y: 7 }));
        assert_eq!(conn.recv().unwrap(), Some(Message::Disconnect));

        sender_handle.join().unwrap();
    }

    /// Test that EOF (peer closes connection) returns Ok(None).
    #[test]
    fn recv_eof_returns_none() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let sender_handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            // Drop the stream immediately to simulate disconnect
            drop(stream);
        });

        let addr = format!("127.0.0.1:{}", port);
        let stream = TcpStream::connect(&addr).unwrap();
        let mut conn = Connection::from_stream(stream).unwrap();

        // Give the sender thread time to close
        sender_handle.join().unwrap();

        // Should get None (EOF), not an error
        assert_eq!(conn.recv().unwrap(), None);
    }
}
