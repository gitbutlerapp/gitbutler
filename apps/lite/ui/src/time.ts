/** @public */
export const formatRelativeTimeWith =
	(rtf: Intl.RelativeTimeFormat) =>
	(timestamp: number, now = Date.now()): string => {
		const seconds = Math.round((timestamp - now) / 1000);
		const absSeconds = Math.abs(seconds);

		if (absSeconds < 60) return rtf.format(seconds, "seconds");
		if (absSeconds < 60 * 60) return rtf.format(Math.round(seconds / 60), "minutes");
		if (absSeconds < 60 * 60 * 24) return rtf.format(Math.round(seconds / 60 / 60), "hours");
		if (absSeconds < 60 * 60 * 24 * 30)
			return rtf.format(Math.round(seconds / 60 / 60 / 24), "days");
		if (absSeconds < 60 * 60 * 24 * 365)
			return rtf.format(Math.round(seconds / 60 / 60 / 24 / 30), "months");
		return rtf.format(Math.round(seconds / 60 / 60 / 24 / 365), "years");
	};

const stdRelativeTimeFormatter = new Intl.RelativeTimeFormat(undefined, {
	numeric: "always",
	style: "long",
});

export const formatRelativeTime: (timestamp: number, now?: number) => string =
	formatRelativeTimeWith(stdRelativeTimeFormatter);

/** @public */
export const formatDurationWith =
	(df: Intl.DurationFormat) =>
	(ms: number): string => {
		const sign = Math.sign(ms);
		let msRemaining = Math.round(Math.abs(ms));

		const weeks = Math.trunc(msRemaining / 604_800_000);
		msRemaining %= 604_800_000;
		const days = Math.trunc(msRemaining / 86_400_000);
		msRemaining %= 86_400_000;
		const hours = Math.trunc(msRemaining / 3_600_000);
		msRemaining %= 3_600_000;
		const minutes = Math.trunc(msRemaining / 60_000);
		msRemaining %= 60_000;
		const seconds = Math.trunc(msRemaining / 1_000);
		msRemaining %= 1_000;

		return df.format({
			weeks: weeks * sign,
			days: days * sign,
			hours: hours * sign,
			minutes: minutes * sign,
			seconds: seconds * sign,
			milliseconds: msRemaining * sign,
		});
	};

const stdDurationFormatter = new Intl.DurationFormat(undefined, { style: "long" });

export const formatDuration: (ms: number) => string = formatDurationWith(stdDurationFormatter);

/**
 * A duration rounded to its largest whole unit, for places with room for one
 * number and nothing more ("12 min", "1 hr"). Sub-minute durations round up to
 * a second so a fast job doesn't read as having taken no time at all.
 *
 * @public
 */
export const formatCompactDurationWith =
	(df: Intl.DurationFormat) =>
	(ms: number): string => {
		// Round before choosing the unit: picking first lets a value that rounds
		// up to a full unit render as "60 sec" or "60 min".
		const seconds = Math.max(1, Math.round(ms / 1_000));
		if (seconds < 60) return df.format({ seconds });

		const minutes = Math.round(ms / 60_000);
		if (minutes < 60) return df.format({ minutes });

		return df.format({ hours: Math.round(ms / 3_600_000) });
	};

const stdCompactDurationFormatter = new Intl.DurationFormat(undefined, { style: "short" });

export const formatCompactDuration: (ms: number) => string = formatCompactDurationWith(
	stdCompactDurationFormatter,
);

const stdAbsoluteTimeFormatter = new Intl.DateTimeFormat(undefined, {
	dateStyle: "medium",
	timeStyle: "short",
});

export const formatAbsoluteTime = (timestamp: number): string =>
	stdAbsoluteTimeFormatter.format(timestamp);
