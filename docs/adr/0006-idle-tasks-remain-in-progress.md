# Started Tasks remain In Progress while idle

Status: accepted (2026-10-11)

Ready means work has not begun; In Progress means work has begun but has not been completed, whether or not work is actively happening at this moment. We deliberately do not add a Paused status or automatically move inactive Tasks back to Ready, because those interpretations confuse task lifecycle with momentary activity and would break the user's always-on-top ordering of started work. Progress, Next Step, and dated history stay intact through idle periods. A future explicit reset/start-over workflow must be deliberate and cannot silently erase history.
