use futures_util::{stream, StreamExt};
use std::future::Future;

/// Runs work with a fixed concurrency ceiling while preserving input order.
///
/// All started futures are awaited before an error is returned. This matters
/// for update checks because some futures own joined blocking Git workers that
/// must not be detached when a peer reports an error.
pub(crate) async fn run_bounded_ordered<I, O, E, F, Fut>(
    items: impl IntoIterator<Item = I>,
    max_concurrency: usize,
    operation: F,
) -> Result<Vec<O>, E>
where
    F: Fn(I) -> Fut,
    Fut: Future<Output = Result<O, E>>,
{
    let mut completed = stream::iter(items.into_iter().enumerate())
        .map(|(index, item)| {
            let future = operation(item);
            async move { (index, future.await) }
        })
        .buffer_unordered(max_concurrency.max(1))
        .collect::<Vec<_>>()
        .await;

    completed.sort_unstable_by_key(|(index, _)| *index);
    completed.into_iter().map(|(_, result)| result).collect()
}

#[cfg(test)]
mod tests {
    use super::run_bounded_ordered;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };
    use tokio::sync::{mpsc, Notify};

    #[tokio::test]
    async fn replenishes_the_pool_without_waiting_for_a_fixed_batch() {
        let active = Arc::new(AtomicUsize::new(0));
        let maximum_active = Arc::new(AtomicUsize::new(0));
        let gates = Arc::new([Notify::new(), Notify::new(), Notify::new(), Notify::new()]);
        let (started_tx, mut started_rx) = mpsc::unbounded_channel();

        let scheduler = tokio::spawn({
            let active = Arc::clone(&active);
            let maximum_active = Arc::clone(&maximum_active);
            let gates = Arc::clone(&gates);
            async move {
                run_bounded_ordered(0..4, 2, move |item| {
                    let active = Arc::clone(&active);
                    let maximum_active = Arc::clone(&maximum_active);
                    let gates = Arc::clone(&gates);
                    let started_tx = started_tx.clone();
                    async move {
                        let now_active = active.fetch_add(1, Ordering::AcqRel) + 1;
                        maximum_active.fetch_max(now_active, Ordering::AcqRel);
                        started_tx.send(item).unwrap();
                        gates[item].notified().await;
                        active.fetch_sub(1, Ordering::AcqRel);
                        Ok::<_, ()>(item)
                    }
                })
                .await
            }
        });

        assert_eq!(started_rx.recv().await, Some(0));
        assert_eq!(started_rx.recv().await, Some(1));

        gates[0].notify_one();
        assert_eq!(started_rx.recv().await, Some(2));
        assert_eq!(maximum_active.load(Ordering::Acquire), 2);

        gates[1].notify_one();
        gates[2].notify_one();
        assert_eq!(started_rx.recv().await, Some(3));
        gates[3].notify_one();

        assert_eq!(scheduler.await.unwrap().unwrap(), vec![0, 1, 2, 3]);
        assert_eq!(maximum_active.load(Ordering::Acquire), 2);
    }

    #[tokio::test]
    async fn waits_for_started_work_before_returning_an_error() {
        let second_started = Arc::new(Notify::new());
        let release_second = Arc::new(Notify::new());
        let second_completed = Arc::new(AtomicBool::new(false));

        let scheduler = tokio::spawn({
            let second_started = Arc::clone(&second_started);
            let release_second = Arc::clone(&release_second);
            let second_completed = Arc::clone(&second_completed);
            async move {
                run_bounded_ordered([0, 1], 2, move |item| {
                    let second_started = Arc::clone(&second_started);
                    let release_second = Arc::clone(&release_second);
                    let second_completed = Arc::clone(&second_completed);
                    async move {
                        if item == 0 {
                            Err("failed")
                        } else {
                            second_started.notify_one();
                            release_second.notified().await;
                            second_completed.store(true, Ordering::Release);
                            Ok(item)
                        }
                    }
                })
                .await
            }
        });

        second_started.notified().await;
        tokio::task::yield_now().await;
        assert!(!scheduler.is_finished());
        assert!(!second_completed.load(Ordering::Acquire));

        release_second.notify_one();
        assert_eq!(scheduler.await.unwrap(), Err("failed"));
        assert!(second_completed.load(Ordering::Acquire));
    }
}
