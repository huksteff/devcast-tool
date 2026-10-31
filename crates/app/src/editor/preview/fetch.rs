//! Which frame the preview asks the render thread for and which answer it shows. Every new wish
//! (project revision, moment, size) is asked at once with the next generation number; the thread
//! works only on the newest request, and answers come in the order of requests. The last frame
//! stays shown until a newer one arrives, so the preview never goes blank while it waits.

use dc_core::{OutputTime, PixelSize};

/// The frame the preview wants: of this project revision, at this output moment, this large,
/// exact or quick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Want {
    pub revision: u64,
    pub time: OutputTime,
    pub size: PixelSize,
    /// The playhead is being dragged: the keyframe before `time` will do (decision 003).
    pub scrub: bool,
}

/// What was asked and what is shown; `F` is a rendered frame.
#[derive(Debug)]
pub struct Fetch<F> {
    generation: u64,
    asked: Option<Want>,
    /// Generation of the answer shown (frame or error).
    answered: u64,
    shown: Option<F>,
    error: Option<String>,
}

impl<F> Default for Fetch<F> {
    fn default() -> Self {
        Self {
            generation: 0,
            asked: None,
            answered: 0,
            shown: None,
            error: None,
        }
    }
}

impl<F> Fetch<F> {
    /// The generation to request `want` with, or `None` when it is what was asked last.
    pub fn want(&mut self, want: Want) -> Option<u64> {
        if self.asked == Some(want) {
            return None;
        }
        self.asked = Some(want);
        self.generation += 1;
        Some(self.generation)
    }

    /// The answer to request `generation`: a frame replaces the shown one, an error is shown
    /// instead of it. An answer older than the shown one is dropped. Returns whether the
    /// preview changed.
    pub fn answer(&mut self, generation: u64, answer: Result<F, String>) -> bool {
        if generation <= self.answered {
            return false;
        }
        self.answered = generation;
        match answer {
            Ok(frame) => {
                self.shown = Some(frame);
                self.error = None;
            }
            Err(error) => {
                self.shown = None;
                self.error = Some(error);
            }
        }
        true
    }

    /// The thread could not take the request: the frame is lost, the next wish asks again.
    pub fn lost(&mut self, error: String) {
        self.asked = None;
        self.answered = self.generation;
        self.shown = None;
        self.error = Some(error);
    }

    pub fn shown(&self) -> Option<&F> {
        self.shown.as_ref()
    }

    /// Why there is no frame.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn want(revision: u64, secs: f64, width: u32) -> Want {
        Want {
            revision,
            time: OutputTime::from_secs_f64(secs),
            size: PixelSize {
                width,
                height: width / 2,
            },
            scrub: false,
        }
    }

    /// A render thread that answers the newest waiting request only, like the real one.
    #[derive(Default)]
    struct FakeThread {
        waiting: Vec<(u64, Want)>,
    }

    impl FakeThread {
        fn ask(&mut self, fetch: &mut Fetch<Want>, want: Want) {
            if let Some(generation) = fetch.want(want) {
                self.waiting.push((generation, want));
            }
        }

        /// Renders the newest request; the older waiting ones are skipped.
        fn render(&mut self, fetch: &mut Fetch<Want>) -> bool {
            let newest = self.waiting.pop();
            self.waiting.clear();
            newest.is_some_and(|(generation, want)| fetch.answer(generation, Ok(want)))
        }
    }

    #[test]
    fn the_same_wish_is_asked_once() {
        let (mut fetch, mut thread) = (Fetch::default(), FakeThread::default());
        thread.ask(&mut fetch, want(1, 0.0, 640));
        thread.ask(&mut fetch, want(1, 0.0, 640));
        assert_eq!(thread.waiting.len(), 1);
        assert!(thread.render(&mut fetch));
        thread.ask(&mut fetch, want(1, 0.0, 640));
        assert!(
            thread.waiting.is_empty(),
            "the shown frame is not asked again"
        );
    }

    #[test]
    fn time_size_and_revision_each_ask_again() {
        let (mut fetch, mut thread) = (Fetch::default(), FakeThread::default());
        for wish in [
            want(1, 0.0, 640),
            want(1, 2.5, 640),
            want(1, 2.5, 320),
            want(2, 2.5, 320),
        ] {
            thread.ask(&mut fetch, wish);
            assert!(thread.render(&mut fetch));
            assert_eq!(fetch.shown(), Some(&wish));
        }
    }

    #[test]
    fn the_last_frame_stays_until_the_newest_arrives() {
        let (mut fetch, mut thread) = (Fetch::default(), FakeThread::default());
        thread.ask(&mut fetch, want(1, 0.0, 640));
        thread.render(&mut fetch);
        thread.ask(&mut fetch, want(1, 1.0, 640));
        thread.ask(&mut fetch, want(1, 2.0, 640));
        assert_eq!(
            fetch.shown(),
            Some(&want(1, 0.0, 640)),
            "shown while waiting"
        );
        assert_eq!(thread.waiting.len(), 2);
        thread.render(&mut fetch);
        assert_eq!(fetch.shown(), Some(&want(1, 2.0, 640)), "1.0 was skipped");
    }

    #[test]
    fn an_older_answer_never_replaces_a_newer_frame() {
        let mut fetch = Fetch::default();
        let first = fetch.want(want(1, 1.0, 640)).unwrap();
        let second = fetch.want(want(1, 2.0, 640)).unwrap();
        assert!(fetch.answer(second, Ok(want(1, 2.0, 640))));
        assert!(!fetch.answer(first, Ok(want(1, 1.0, 640))));
        assert_eq!(fetch.shown(), Some(&want(1, 2.0, 640)));
    }

    #[test]
    fn an_answer_on_the_way_is_shown_before_the_newest() {
        let mut fetch = Fetch::default();
        let first = fetch.want(want(1, 1.0, 640)).unwrap();
        let second = fetch.want(want(1, 2.0, 640)).unwrap();
        assert!(fetch.answer(first, Ok(want(1, 1.0, 640))));
        assert!(
            fetch.answer(second, Ok(want(1, 2.0, 640))),
            "2.0 still comes"
        );
    }

    #[test]
    fn an_error_takes_the_place_of_the_frame_until_a_frame_comes() {
        let mut fetch = Fetch::default();
        let first = fetch.want(want(1, 1.0, 640)).unwrap();
        fetch.answer(first, Ok(want(1, 1.0, 640)));
        let broken = fetch.want(want(2, 1.0, 640)).unwrap();
        assert!(fetch.answer(broken, Err("нет клипов".to_owned())));
        assert_eq!((fetch.shown(), fetch.error()), (None, Some("нет клипов")));
        let fixed = fetch.want(want(3, 1.0, 640)).unwrap();
        fetch.answer(fixed, Ok(want(3, 1.0, 640)));
        assert_eq!(fetch.error(), None);
        assert!(fetch.shown().is_some());
    }

    #[test]
    fn a_lost_request_is_asked_again() {
        let mut fetch: Fetch<Want> = Fetch::default();
        fetch.want(want(1, 1.0, 640));
        fetch.lost("поток превью остановился".to_owned());
        assert!(fetch.want(want(1, 1.0, 640)).is_some());
    }
}
