use michiu_ui::{MichiuTrace, TraceSubscription};

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
                match &record.trace {
                    MichiuTrace::Error { detail, .. } => {
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
                    #[cfg(feature = "trace-lifecycle")]
                    MichiuTrace::BuildElement {
                        old, current, root, ..
                    } => {
                        println!(
                            "\n[{frame} Build]\n\
                             - Entity : {entity:?}\n\
                             - Time   : {time:?}\n\
                             - Loc    : {loc}\n\
                             - Func   : {func}\n\
                             - Old    : {old:?}\n\
                             - Curr   : {current:?}\n\
                             - root   : {root:?}\n\
                            "
                        );
                    }
                    #[cfg(feature = "trace-lifecycle")]
                    MichiuTrace::Context { current, state, .. } => {
                        println!(
                            "\n[{frame} Context]\n\
                             - Entity : {entity:?}\n\
                             - Time   : {time:?}\n\
                             - Loc    : {loc}\n\
                             - Func   : {func}\n\
                             - Curr   : {current:?}\n\
                             - State  : {state:?}\n\
                            "
                        );
                    }
                    #[cfg(feature = "trace-lifecycle")]
                    MichiuTrace::Init { .. } => {
                        println!(
                            "\n[{frame} Init]\n\
                             - Entity : {entity:?}\n\
                             - Time   : {time:?}\n\
                             - Loc    : {loc}\n\
                             - Func   : {func}\n\
                            "
                        );
                    }
                    #[cfg(feature = "trace-lifecycle")]
                    MichiuTrace::Animation { kinds, add } => {
                        println!(
                            "\n[{frame} Animation]\n\
                             - Entity : {entity:?}\n\
                             - Time   : {time:?}\n\
                             - Loc    : {loc}\n\
                             - Func   : {func}\n\
                             - Kinds  : {kinds:?}\n\
                             - Add    : {add:?}\n\
                            "
                        );
                    }
                    #[cfg(feature = "trace-lifecycle")]
                    MichiuTrace::Layout { stage, add } => {
                        println!(
                            "\n[{frame} Layout - {stage:?}]\n\
                             - Entity : {entity:?}\n\
                             - Time   : {time:?}\n\
                             - Loc    : {loc}\n\
                             - Func   : {func}\n\
                             - Add    : {add:?}\n\
                            "
                        );
                    }
                    #[cfg(feature = "trace-lifecycle")]
                    MichiuTrace::PrepareRender { stage, add, .. } => {
                        println!(
                            "\n[{frame} PrepareRender - {stage:?}]\n\
                             - Entity : {entity:?}\n\
                             - Time   : {time:?}\n\
                             - Loc    : {loc}\n\
                             - Func   : {func}\n\
                             - Add    : {add:?}\n\
                             - Data   : 多いので無し\n\
                            "
                        );
                    }
                    #[cfg(feature = "trace-lifecycle")]
                    MichiuTrace::WriteBuffer { staging, add } => {
                        let staging = staging.len();
                        println!(
                            "\n[{frame} WriteBuffer]\n\
                             - Entity  : {entity:?}\n\
                             - Time    : {time:?}\n\
                             - Loc     : {loc}\n\
                             - Func    : {func}\n\
                             - Staging : {staging:?}\n\
                             - Add     : {add:?}\n\
                            "
                        );
                    }
                    #[cfg(feature = "trace-lifecycle")]
                    MichiuTrace::Present { add } => {
                        println!(
                            "\n[{frame} Present]\n\
                             - Entity  : {entity:?}\n\
                             - Time    : {time:?}\n\
                             - Loc     : {loc}\n\
                             - Func    : {func}\n\
                             - Add     : {add:?}\n\
                            "
                        );
                    }
                    #[cfg(feature = "trace-lifecycle")]
                    MichiuTrace::Commit { add } => {
                        println!(
                            "\n[{frame} Commit]\n\
                             - Entity  : {entity:?}\n\
                             - Time    : {time:?}\n\
                             - Loc     : {loc}\n\
                             - Func    : {func}\n\
                             - Add     : {add:?}\n\
                            "
                        );
                    }
                    #[cfg(feature = "trace-lifecycle")]
                    MichiuTrace::ClearDirtyEntities {
                        total_entities,
                        active_entities,
                        dirty_layouts,
                        dirty_renders,
                        add,
                    } => {
                        println!(
                            "\n[{frame} ClearDirtyEntities]\n\
                             - Entity  : {entity:?}\n\
                             - Time    : {time:?}\n\
                             - Loc     : {loc}\n\
                             - Func    : {func}\n\
                             - Add     : {add:?}\n\
                             - [Total] {total_entities} | [Active] {active_entities}\n\
                             - [Layouts] {dirty_layouts} | [Renders] {dirty_renders}\n\
                            "
                        );
                    }
                    _ => {}
                }
            }
        }
    });
}
