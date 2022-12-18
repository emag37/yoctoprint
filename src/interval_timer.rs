pub struct IntervalTimer {
    pub interval : std::time::Duration,
    pub last_timeout : std::time::Instant
}

impl IntervalTimer {
    pub fn new(interval: std::time::Duration) -> Self {
        IntervalTimer {interval, last_timeout: std::time::Instant::now() - interval}
    }

    pub fn check(&mut self) -> bool {
        if std::time::Instant::now() >= self.last_timeout + self.interval {
            self.last_timeout = std::time::Instant::now();
            return true;
        }
        return false;
    }
}