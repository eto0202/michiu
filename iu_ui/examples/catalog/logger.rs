use michiu::ui::{MichiuTrace, TraceSubscription};

pub(crate) fn logger(sub: TraceSubscription) {
    std::thread::spawn(move || {
        while let Ok(batch) = sub.recv() {
            use std::io::Write;
            let mut stderr = std::io::stderr().lock();

            for record in batch.iter() {
                let (frame, entity, time, loc, func) = (
                    record.frame,
                    record.id,
                    record.time,
                    record.loc,
                    record.func,
                );
                if let MichiuTrace::Error { detail, .. } = &record.trace {
                    let _ = writeln!(
                        stderr,
                        "\n[{frame} Error]\n\
                         - Entity : {entity:?}\n\
                         - Time   : {time:?}\n\
                         - Loc    : {loc}\n\
                         - Func   : {func}\n\
                         - Detail :\n  \
                           {detail}"
                    );
                }
            }
        }
    });
}
