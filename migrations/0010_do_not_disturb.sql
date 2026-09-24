-- Do not disturb: a snooze, and weekly quiet hours in the user's time zone.
-- A schedule is set when dnd_start is not null; the four schedule columns are
-- set or cleared together.
ALTER TABLE users
    ADD COLUMN dnd_until     TIMESTAMPTZ,
    ADD COLUMN dnd_start     TIME,
    ADD COLUMN dnd_end       TIME,
    ADD COLUMN dnd_days      SMALLINT,
    ADD COLUMN dnd_time_zone TEXT,
    ADD CONSTRAINT users_dnd_schedule_complete CHECK (
        (dnd_start IS NULL) = (dnd_end IS NULL)
        AND (dnd_start IS NULL) = (dnd_days IS NULL)
        AND (dnd_start IS NULL) = (dnd_time_zone IS NULL)
    );
