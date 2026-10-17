-- Counting copies of Telex whose users said yes (crates/telex-core/src/count.rs), and daily
-- snapshots of GitHub's download totals. Only sums: no ID, no address, no row per copy.
-- The site's server writes through the functions below; browsers have no access.

-- One row per UTC day and kind of copy. Each copy sends at most one count a day, so `copies` is
-- the day's active copies; `weekly` and `monthly` count only a copy's first count of its ISO week
-- or month, so their sums over a week or a month are that period's active copies.
create table if not exists public.telex_count_days (
  day date not null,
  version text not null check (version ~ '^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,5}$'),
  os text not null check (os in ('windows', 'appimage', 'linux')),
  lang text not null check (lang in ('en', 'pl')),
  -- The month the copy was first counted: cohorts for "still used three months later".
  since text not null check (since ~ '^20[0-9]{2}-(0[1-9]|1[0-2])$'),
  copies integer not null default 0,
  weekly integer not null default 0,
  monthly integer not null default 0,
  new integer not null default 0,
  primary key (day, version, os, lang, since)
);
alter table public.telex_count_days enable row level security;
revoke all on public.telex_count_days from anon, authenticated, service_role;
grant select, insert, update on public.telex_count_days to service_role;

create or replace function public.telex_count_add(
  p_version text, p_os text, p_lang text, p_since text, p_new boolean, p_week boolean, p_month boolean
) returns void language sql volatile security invoker set search_path = public as $$
  insert into public.telex_count_days as c (day, version, os, lang, since, copies, weekly, monthly, new)
  values ((now() at time zone 'UTC')::date, p_version, p_os, p_lang, p_since, 1,
          p_week::int, p_month::int, p_new::int)
  on conflict (day, version, os, lang, since) do update set
    copies = c.copies + 1,
    weekly = c.weekly + excluded.weekly,
    monthly = c.monthly + excluded.monthly,
    new = c.new + excluded.new;
$$;
revoke all on function public.telex_count_add(text, text, text, text, boolean, boolean, boolean) from public, anon, authenticated;
grant execute on function public.telex_count_add(text, text, text, text, boolean, boolean, boolean) to service_role;

-- GitHub keeps only a running total per release file. A snapshot each day (the Vercel cron in
-- vercel.json) turns the totals into downloads per day.
create table if not exists public.telex_download_totals (
  day date not null,
  release text not null check (char_length(release) <= 40),
  asset text not null check (char_length(asset) <= 200),
  downloads integer not null check (downloads >= 0),
  primary key (day, asset)
);
alter table public.telex_download_totals enable row level security;
revoke all on public.telex_download_totals from anon, authenticated, service_role;
grant select, insert, update on public.telex_download_totals to service_role;
