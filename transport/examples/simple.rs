use std::{sync::{atomic::{AtomicUsize, Ordering}, mpsc::channel, Arc}, thread, time::{Duration, Instant}};

fn main() {
    let (tx, rx) = channel();
    thread::spawn(move || {
        let mut flag = 0;

        loop {
            tx.send((flag, Instant::now())).unwrap();
            flag = if flag == 0 {
                1
            } else {
                0
            };

            thread::sleep(Duration::from_millis(5));
        }
    });

    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();
    thread::spawn(move || {
        while let Ok((flag, time)) = rx.recv() {
            if flag == 0 {
                tx1.send(time).unwrap();
            } else {
                tx2.send(time).unwrap();
            }
        }
    });

    let delay = Arc::new(AtomicUsize::new(0));

    let delay_ = delay.clone();
    thread::spawn(move || {
        while let Ok(time) = rx1.recv() {
            delay_.store(time.elapsed().as_micros() as usize, Ordering::Relaxed);
        }
    });

    let delay_ = delay.clone();
    thread::spawn(move || {
        while let Ok(time) = rx2.recv() {
            delay_.store(time.elapsed().as_micros() as usize, Ordering::Relaxed);
        }
    });

    loop {
        println!("{}", delay.load(Ordering::Relaxed));
        thread::sleep(Duration::from_secs(1));
    }
}
