use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    thread,
    time::Duration,
};

use futures::{task::AtomicWaker, Stream};
use sync::atomic::AtomicOption;

use crate::{api, inetv4_addr, Packet, Rtp, RtpConfig, RtpError, RtpErrorKind};

struct RContext {
    packet: AtomicOption<Packet>,
    waker: AtomicWaker,
    rtp: Arc<Rtp>,
}

/// Receiver of RTP packets that can receive point-to-point and broadcast
/// packets.
pub struct RtpReceiver {
    ctx: Arc<RContext>,
}

impl RtpReceiver {
    /// To create a receiver, you need to specify a port number for receiving
    /// packets.
    pub fn new(cfg: RtpConfig) -> Result<Self, RtpError> {
        let ptr = unsafe {
            api::create_receiver(
                inetv4_addr(&cfg.bind)?,
                cfg.bind.port(),
                inetv4_addr(&cfg.dest)?,
                cfg.dest.port(),
            )
        };
        
        if ptr.is_null() {
            return RtpError::error(RtpErrorKind::UnableToCreateRTP);
        }

        let ctx = Arc::new(RContext {
            rtp: Arc::new(Rtp::new(ptr)),
            packet: AtomicOption::new(None),
            waker: AtomicWaker::new(),
        });

        // This is a background thread that is used to keep fetching packets and
        // informing the async stream future about all the packets it has fetched.
        let ctx_ = Arc::downgrade(&ctx);
        thread::spawn(move || loop {
            if let Some(ctx) = ctx_.upgrade() {
                // Notify the underlying locked cache queue to allow us to process the data.
                if !unsafe { api::lock_poll_thread(ctx.rtp.as_raw()) } {
                    break;
                }

                // Try to get the first data source.
                if unsafe { api::goto_first_source(ctx.rtp.as_raw()) } {
                    loop {
                        // Get the packet and wake up future if it exists.
                        let pkt = unsafe { api::get_next_packet(ctx.rtp.as_raw()) };
                        if !pkt.is_null() {
                            ctx.packet.swap(Some(Packet::new(ctx.rtp.clone(), pkt)));
                            ctx.waker.wake();
                        }

                        // Proceed to check if the data exists in the next source.
                        if !unsafe { api::goto_next_source(ctx.rtp.as_raw()) } {
                            break;
                        }
                    }
                }

                // No more data needs to be processed, notify the underlying layer to unlock the
                // buffer queue.
                if !unsafe { api::unlock_poll_thread(ctx.rtp.as_raw()) } {
                    break;
                }

                // Don't let the loop idle too fast, it's a pointless thing to do, here let the
                // loop slow down.
                thread::sleep(Duration::from_millis(20));
            } else {
                break;
            }
        });

        Ok(Self { ctx })
    }
}

impl Stream for RtpReceiver {
    type Item = Packet;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.ctx.waker.register(cx.waker());
        self.ctx
            .packet
            .swap(None)
            .map(|pkt| Poll::Ready(Some(pkt)))
            .unwrap_or_else(|| Poll::Pending)
    }
}
