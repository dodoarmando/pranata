import { useQuery } from "@tanstack/react-query";
import { createFileRoute, useRouteContext } from "@tanstack/react-router";
import { ChevronDownIcon, DownloadIcon } from "lucide-react";
import { type ReactNode, useState } from "react";
import { RequirePermission } from "@/admin/require-permission";
import { ApiError } from "@/api/fetcher";
import {
	getExportAuditEntriesUrl,
	useListAuditEntries,
} from "@/api/generated/audit/audit";
import type {
	AuditRow,
	ListAuditEntriesParams,
	UserSummary,
} from "@/api/generated/model";
import {
	getGetPublicVillageQueryKey,
	getPublicVillage,
} from "@/api/generated/public/public";
import { useListUsers } from "@/api/generated/users/users";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { ACTION_LABELS, actionLabel, objectLabel } from "@/lib/audit-labels";
import { formatAuditTime } from "@/lib/audit-time";
import { messageFor } from "@/lib/errors";
import { can } from "@/lib/permissions";

export const Route = createFileRoute("/admin/sistem/log-audit")({
	head: () => ({ meta: [{ title: "Log audit · Admin Pranata" }] }),
	component: () => (
		<RequirePermission permission="audit.read">
			<AuditLogPage />
		</RequirePermission>
	),
});

const ALL = "all";
const FALLBACK_TIME_ZONE = "Asia/Jakarta";

type Filters = Pick<ListAuditEntriesParams, "actor" | "action" | "from" | "to">;

function errorMessage(error: unknown): string {
	return messageFor(
		error instanceof ApiError ? error.messageCode : "internal_error",
	);
}

// Empty filters are left out, so they never reach the URL.
function toParams(
	actor: string,
	action: string,
	from: string,
	to: string,
): Filters {
	return {
		...(actor !== ALL && { actor }),
		...(action !== ALL && { action }),
		...(from && { from }),
		...(to && { to }),
	};
}

function ChangesTable({ row }: Readonly<{ row: AuditRow }>) {
	return (
		<Table>
			<TableHeader>
				<TableRow>
					<TableHead>Kolom</TableHead>
					<TableHead>Sebelum</TableHead>
					<TableHead>Sesudah</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{row.changes.map((change) => (
					<TableRow key={change.field}>
						<TableCell className="whitespace-normal break-words">
							{change.field}
						</TableCell>
						<TableCell className="whitespace-normal break-all text-muted-foreground line-through">
							{show(change.before)}
						</TableCell>
						<TableCell className="whitespace-normal break-all">
							{show(change.after)}
						</TableCell>
					</TableRow>
				))}
			</TableBody>
		</Table>
	);
}

function show(value: unknown): string {
	if (value === null || value === undefined) return "—";
	return typeof value === "string" ? value : JSON.stringify(value);
}

function EntryRow({
	row,
	timeZone,
}: Readonly<{ row: AuditRow; timeZone: string }>) {
	const destructive = row.action === "deactivate" || row.action === "delete";
	const summary: ReactNode = (
		<>
			<span className="w-36 text-muted-foreground">
				{formatAuditTime(row.at, timeZone)}
			</span>
			<span className="w-36 font-medium">
				{row.actor?.display_name ?? "Sistem"}
			</span>
			<span className="w-44">
				<Badge
					variant={destructive ? "destructive" : "outline"}
					// The stock destructive text is 4.1:1 on its tint; AA needs 4.5.
					className={destructive ? "text-red-700 dark:text-red-400" : undefined}
				>
					{actionLabel(row.action)}
				</Badge>
			</span>
			<span className="min-w-48 flex-1 break-words">
				{objectLabel(row.object_type)} · {row.summary}
			</span>
		</>
	);
	const layout = "flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3.5";
	if (row.changes.length === 0) {
		return (
			<li className="border-b last:border-b-0">
				<div className={layout}>{summary}</div>
			</li>
		);
	}
	return (
		<li className="border-b last:border-b-0">
			<details className="group">
				<summary
					className={`${layout} cursor-pointer list-none [&::-webkit-details-marker]:hidden`}
				>
					{summary}
					<ChevronDownIcon
						aria-hidden="true"
						className="size-4 text-muted-foreground transition-transform group-open:rotate-180"
					/>
					<span className="sr-only">Lihat perubahan</span>
				</summary>
				<div className="px-4 pb-3.5">
					<ChangesTable row={row} />
				</div>
			</details>
		</li>
	);
}

// One page of entries, as the `<li>`s of the list that holds every page.
function EntryPage({
	params,
	before,
	timeZone,
	isLast,
	onMore,
}: Readonly<{
	params: Filters;
	before: string | undefined;
	timeZone: string;
	isLast: boolean;
	onMore: (next: string) => void;
}>) {
	const page = useListAuditEntries({ ...params, ...(before && { before }) });
	if (page.error) {
		return (
			<li role="alert" className="px-4 py-3.5 text-destructive">
				{errorMessage(page.error)}
			</li>
		);
	}
	if (!page.data) {
		return (
			<li className="p-4">
				<Skeleton className="h-16 w-full" />
			</li>
		);
	}
	const { entries, next } = page.data;
	return (
		<>
			{entries.length === 0 && before === undefined ? (
				<li className="px-4 py-3.5 text-sm text-muted-foreground">
					Belum ada catatan untuk filter ini.
				</li>
			) : null}
			{entries.map((row) => (
				<EntryRow key={row.id} row={row} timeZone={timeZone} />
			))}
			{isLast && next ? (
				<li className="flex justify-center p-3">
					<Button type="button" variant="outline" onClick={() => onMore(next)}>
						Muat lebih banyak
					</Button>
				</li>
			) : null}
		</>
	);
}

function EntryList({
	params,
	timeZone,
}: Readonly<{ params: Filters; timeZone: string }>) {
	// The cursor each loaded page after the first starts from.
	const [cursors, setCursors] = useState<string[]>([]);
	const befores = [undefined, ...cursors];
	return (
		<ul className="rounded-lg border">
			{befores.map((before, index) => (
				<EntryPage
					key={before ?? "first"}
					params={params}
					before={before}
					timeZone={timeZone}
					isLast={index === befores.length - 1}
					onMore={(next) => setCursors((all) => [...all, next])}
				/>
			))}
		</ul>
	);
}

function AuditLogPage() {
	const { session } = useRouteContext({ from: "/admin" });
	const [actor, setActor] = useState(ALL);
	const [action, setAction] = useState(ALL);
	const [from, setFrom] = useState("");
	const [to, setTo] = useState("");
	const params = toParams(actor, action, from, to);

	const canPickUser = can(session, "user.manage");
	const users = useListUsers({ query: { enabled: canPickUser } });
	// Not the generated hook: the public home imports this module too, and a
	// hook used here would pull react-query into the public bundle.
	const village = useQuery({
		queryKey: getGetPublicVillageQueryKey(),
		queryFn: ({ signal }) => getPublicVillage({ signal }),
	});
	const userItems = [
		{ value: ALL, label: "Semua pengguna" },
		...(users.data ?? []).map((user: UserSummary) => ({
			value: user.id,
			label: user.display_name,
		})),
	];
	const actionItems = [
		{ value: ALL, label: "Semua tindakan" },
		...Object.entries(ACTION_LABELS).map(([value, label]) => ({
			value,
			label,
		})),
	];
	const timeZone = village.data?.time_zone ?? FALLBACK_TIME_ZONE;

	return (
		<div className="mx-auto flex max-w-5xl flex-col gap-4">
			<div className="flex flex-wrap items-start justify-between gap-3">
				<div className="grid gap-1">
					<h2 className="text-2xl font-semibold">Log audit</h2>
					<p className="text-sm text-muted-foreground">
						Setiap perubahan data, siapa yang mengubah, dan kapan.
					</p>
				</div>
				<Button
					variant="outline"
					nativeButton={false}
					render={<a href={getExportAuditEntriesUrl(params)} />}
				>
					<DownloadIcon data-icon="inline-start" />
					Unduh CSV
				</Button>
			</div>

			<div className="flex flex-wrap items-end gap-3">
				{canPickUser ? (
					<Field className="w-auto">
						<FieldLabel className="sr-only">Pengguna</FieldLabel>
						<Select
							items={userItems}
							value={actor}
							onValueChange={(value) => setActor(value ?? ALL)}
						>
							<SelectTrigger aria-label="Pengguna" className="w-48">
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								{userItems.map((item) => (
									<SelectItem key={item.value} value={item.value}>
										{item.label}
									</SelectItem>
								))}
							</SelectContent>
						</Select>
					</Field>
				) : null}
				<Field className="w-auto">
					<FieldLabel className="sr-only">Tindakan</FieldLabel>
					<Select
						items={actionItems}
						value={action}
						onValueChange={(value) => setAction(value ?? ALL)}
					>
						<SelectTrigger aria-label="Tindakan" className="w-48">
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							{actionItems.map((item) => (
								<SelectItem key={item.value} value={item.value}>
									{item.label}
								</SelectItem>
							))}
						</SelectContent>
					</Select>
				</Field>
				<Field className="w-auto">
					<FieldLabel htmlFor="audit-from">Dari</FieldLabel>
					<Input
						id="audit-from"
						type="date"
						value={from}
						onChange={(e) => setFrom(e.target.value)}
					/>
				</Field>
				<Field className="w-auto">
					<FieldLabel htmlFor="audit-to">Sampai</FieldLabel>
					<Input
						id="audit-to"
						type="date"
						value={to}
						onChange={(e) => setTo(e.target.value)}
					/>
				</Field>
			</div>

			{village.isPending ? (
				<Skeleton className="h-64 w-full" />
			) : (
				// A new filter starts again from the first page.
				<EntryList
					key={JSON.stringify(params)}
					params={params}
					timeZone={timeZone}
				/>
			)}
		</div>
	);
}
