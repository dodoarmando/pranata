const MONTHS = [
	"Jan",
	"Feb",
	"Mar",
	"Apr",
	"Mei",
	"Jun",
	"Jul",
	"Agu",
	"Sep",
	"Okt",
	"Nov",
	"Des",
];

const DAY_MS = 24 * 60 * 60 * 1000;

interface Parts {
	year: string;
	month: number;
	day: string;
	hour: string;
	minute: string;
}

function parts(date: Date, timeZone: string): Parts {
	const formatted = new Intl.DateTimeFormat("en-GB", {
		timeZone,
		year: "numeric",
		month: "numeric",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
		hourCycle: "h23",
	}).formatToParts(date);
	const get = (type: string) =>
		formatted.find((p) => p.type === type)?.value ?? "";
	return {
		year: get("year"),
		month: Number(get("month")),
		day: get("day"),
		hour: get("hour"),
		minute: get("minute"),
	};
}

const dayKey = (p: Parts) => `${p.year}-${p.month}-${p.day}`;

// "Hari ini, 10.42", "Kemarin, 08.30" or "18 Nov 2026, 07.58", in the
// Village's time zone (an IANA name).
export function formatAuditTime(
	iso: string,
	timeZone: string,
	now: Date = new Date(),
): string {
	const at = parts(new Date(iso), timeZone);
	const time = `${at.hour}.${at.minute}`;
	const key = dayKey(at);
	if (key === dayKey(parts(now, timeZone))) return `Hari ini, ${time}`;
	if (key === dayKey(parts(new Date(now.getTime() - DAY_MS), timeZone))) {
		return `Kemarin, ${time}`;
	}
	return `${Number(at.day)} ${MONTHS[at.month - 1]} ${at.year}, ${time}`;
}
