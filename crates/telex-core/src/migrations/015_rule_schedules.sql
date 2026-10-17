-- Version 15: rules that run on a schedule (filters.rs), besides new mail or instead of it.

-- 0: the rule doesn't run on new mail (only on its schedule, or by hand with Run now).
ALTER TABLE rules ADD COLUMN on_arrival INTEGER NOT NULL DEFAULT 1;
-- JSON: {"every": "hour" | "day" | "week", "folders": ["account/folder", ...]}, or NULL.
ALTER TABLE rules ADD COLUMN schedule TEXT;
-- When the schedule last ran the rule, local time; NULL before its first run.
ALTER TABLE rules ADD COLUMN last_run TEXT;
