import {
	ArrowRight,
	Question as HelpCircle,
	GitlabLogo,
	Sparkle,
	Keyboard,
	Scissors,
	GearSix as Settings2,
} from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
} from "@/components/ui/dialog";
import { useI18n, useScopedT } from "@/contexts/I18nContext";
import { useShortcuts } from "@/contexts/ShortcutsContext";
import { formatBinding, SHORTCUT_ACTIONS, SHORTCUT_LABELS } from "@/lib/shortcuts";
import { formatShortcut } from "@/utils/platformUtils";
import { toast } from "@/components/ui/toast";
import contactsYaml from "@/config/contacts.yaml?raw";
import changelogMarkdown from "@/config/Changelog.md?raw";

type Contact = { name: string; tag: string };
type LocalizedContacts = { repository: string; contacts: Contact[] };
type ChangelogEntry = { version: string; sections: { title: string; items: string[] }[] };

function parseContactsYaml(source: string): Record<"Eng" | "Ru", LocalizedContacts> {
	const repository = source.match(/^Repository:\s*(.+)$/m)?.[1]?.trim() ?? "";
	const result: Record<"Eng" | "Ru", LocalizedContacts> = {
		Eng: { repository, contacts: [] },
		Ru: { repository, contacts: [] },
	};
	let locale: "Eng" | "Ru" | null = null;
	let pendingName: string | null = null;

	for (const line of source.split(/\r?\n/)) {
		const localeMatch = line.match(/^(Eng|Ru):\s*$/);
		if (localeMatch) {
			locale = localeMatch[1] as "Eng" | "Ru";
			continue;
		}
		if (!locale) continue;
		const nameMatch = line.match(/^\s*-\s*Имя:\s*(.+)$/);
		if (nameMatch) {
			pendingName = nameMatch[1].trim();
			continue;
		}
		const tagMatch = line.match(/^\s*Tag:\s*["']?(.+?)["']?\s*$/);
		if (tagMatch && pendingName) {
			result[locale].contacts.push({ name: pendingName, tag: tagMatch[1].trim() });
			pendingName = null;
		}
	}

	return result;
}

function parseChangelog(source: string): ChangelogEntry[] {
	const entries: ChangelogEntry[] = [];
	let entry: ChangelogEntry | null = null;
	let section: ChangelogEntry["sections"][number] | null = null;
	for (const line of source.split(/\r?\n/)) {
		const version = line.match(/^##\s+(.+)$/);
		if (version) {
			entry = { version: version[1].trim(), sections: [] };
			entries.push(entry);
			section = null;
			continue;
		}
		const heading = line.match(/^###\s+(.+)$/);
		if (heading && entry) {
			section = { title: heading[1].trim(), items: [] };
			entry.sections.push(section);
			continue;
		}
		const item = line.match(/^-\s+(.+)$/);
		if (item && section) section.items.push(item[1].trim());
	}
	return entries;
}

const CONTACTS_BY_LOCALE = parseContactsYaml(contactsYaml);
const CHANGELOG_ENTRIES = parseChangelog(changelogMarkdown);
export const APP_HEADER_ACTION_BUTTON_CLASS =
	"h-7 px-2 text-xs text-muted-foreground hover:bg-foreground/10 hover:text-foreground transition-all gap-1.5";
export const APP_HEADER_ICON_BUTTON_CLASS =
	"h-9 w-9 min-w-9 p-0 text-muted-foreground hover:bg-foreground/10 hover:text-foreground transition-all";

interface KeyboardShortcutsDialogProps {
	triggerLabel?: string;
	triggerClassName?: string;
}

export async function openExternalLink(url: string, errorMessage: string) {
	try {
		const result = await window.electronAPI.openExternalUrl(url);
		if (!result.success) {
			toast.error(result.error || errorMessage);
		}
	} catch (error) {
		toast.error(`${errorMessage} ${String(error)}`);
	}
}

export function FeedbackDialog() {
	const t = useScopedT("editor");
	const { locale } = useI18n();
	const contacts = CONTACTS_BY_LOCALE[locale === "ru" ? "Ru" : "Eng"];
	const copyTag = async (tag: string) => {
		try {
			const nativeClipboard = window.electronAPI?.copyTextToClipboard;
			const result = nativeClipboard
				? await nativeClipboard(tag)
				: (await navigator.clipboard.writeText(tag), { success: true });
			if (!result.success) {
				throw new Error(result.error || "Could not copy tag");
			}
			toast.success(t("feedback.tagCopied", "Tag copied"));
		} catch {
			toast.error(t("feedback.tagCopyFailed", "Couldn't copy tag"));
		}
	};

	return (
		<Dialog>
			<DialogTrigger asChild>
				<Button
					variant="ghost"
					size="sm"
					className={APP_HEADER_ICON_BUTTON_CLASS}
					title={t("feedback.contactsTitle", "Contacts")}
					aria-label={t("feedback.contactsTitle", "Contacts")}
				>
					<HelpCircle className="h-3.5 w-3.5" />
				</Button>
			</DialogTrigger>
			<DialogContent className="max-w-lg">
				<DialogHeader>
					<DialogTitle className="text-xl font-semibold text-foreground flex items-center gap-2">
						<HelpCircle className="h-5 w-5 text-[#2563EB]" />{" "}
						{t("feedback.contactsTitle", "Contacts")}
					</DialogTitle>
				</DialogHeader>
				<div className="mt-2 space-y-4">
					<div className="rounded-xl border border-foreground/10 bg-foreground/[0.03] p-4 space-y-3">
						<p className="text-[11px] uppercase tracking-[0.18em] text-muted-foreground/70">
							{t("feedback.contactsLabel", "Contacts")}
						</p>
						{contacts.contacts.map((contact) => (
							<div key={`${contact.name}-${contact.tag}`} className="rounded-lg border border-foreground/5 bg-foreground/5 px-3 py-3">
								<p className="text-sm font-medium text-foreground">{contact.name}</p>
								<button
									type="button"
									className="mt-1 text-xs text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
									onClick={() => void copyTag(contact.tag)}
								>
									{contact.tag}
								</button>
							</div>
						))}
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}

export function RepositoryLinkButton() {
	const t = useScopedT("editor");
	const { locale } = useI18n();
	const repository = CONTACTS_BY_LOCALE[locale === "ru" ? "Ru" : "Eng"].repository;
	return (
		<Button
			type="button"
			variant="ghost"
			size="sm"
			className={APP_HEADER_ICON_BUTTON_CLASS}
			title={t("feedback.repositoryLabel", "Repository")}
			aria-label={t("feedback.repositoryLabel", "Repository")}
			onClick={() => void openExternalLink(repository, t("feedback.openFailed", "Failed to open link."))}
		>
			<GitlabLogo className="h-3.5 w-3.5" />
		</Button>
	);
}

export function WhatsNewDialog() {
	const t = useScopedT("editor");
	return (
		<Dialog>
			<DialogTrigger asChild>
				<Button variant="ghost" size="sm" className={APP_HEADER_ICON_BUTTON_CLASS} title={t("feedback.whatsNew", "What's new")} aria-label={t("feedback.whatsNew", "What's new")}>
					<Sparkle className="h-3.5 w-3.5" />
				</Button>
			</DialogTrigger>
			<DialogContent className="max-w-lg">
				<DialogHeader>
					<DialogTitle className="text-xl font-semibold text-foreground flex items-center gap-2">
						<Sparkle className="h-5 w-5 text-[#2563EB]" /> {t("feedback.whatsNew", "What's new")}
					</DialogTitle>
				</DialogHeader>
				<div className="mt-2 max-h-[60vh] space-y-6 overflow-y-auto pr-2">
					{CHANGELOG_ENTRIES.map((entry) => (
						<section key={entry.version} className="border-t border-foreground/10 pt-4 first:border-t-0 first:pt-0">
							<h3 className="text-lg font-semibold">{entry.version}</h3>
							{entry.sections.map((section) => (
								<div key={section.title} className="mt-4">
									<h4 className="font-medium">{section.title}</h4>
									<ul className="mt-2 list-disc space-y-1 pl-5 text-sm text-muted-foreground">
										{section.items.map((item) => <li key={item}>{item}</li>)}
									</ul>
								</div>
							))}
						</section>
					))}
				</div>
			</DialogContent>
		</Dialog>
	);
}

export function KeyboardShortcutsDialog({
	triggerLabel,
	triggerClassName,
}: KeyboardShortcutsDialogProps) {
	const { shortcuts, isMac, openConfig } = useShortcuts();
	const t = useScopedT("editor");
	const [scrollLabels, setScrollLabels] = useState({
		pan: "Shift + Scroll",
		zoom: "Ctrl + Scroll",
	});
	const shortcutLabel = (action: (typeof SHORTCUT_ACTIONS)[number]) =>
		t(`keyboardShortcuts.action.${action}`, SHORTCUT_LABELS[action]);
	const navigationShortcuts = [
		{
			label: t("keyboardShortcuts.shortcut.stepFrameBack", "Previous frame"),
			binding: formatBinding({ key: "arrowleft" }, isMac),
		},
		{
			label: t("keyboardShortcuts.shortcut.stepFrameForward", "Next frame"),
			binding: formatBinding({ key: "arrowright" }, isMac),
		},
		{
			label: t("keyboardShortcuts.shortcut.cycleForward", "Next annotation"),
			binding: "Tab",
		},
		{
			label: t("keyboardShortcuts.shortcut.cycleBackward", "Previous annotation"),
			binding: "Shift + Tab",
		},
	];
	const editingShortcuts = [
		{
			label: t("keyboardShortcuts.shortcut.undo", "Undo"),
			binding: formatBinding({ key: "z", ctrl: true }, isMac),
		},
		{
			label: t("keyboardShortcuts.shortcut.redo", "Redo"),
			binding: isMac
				? formatBinding({ key: "z", ctrl: true, shift: true }, isMac)
				: `${formatBinding({ key: "z", ctrl: true, shift: true }, isMac)} / ${formatBinding({ key: "y", ctrl: true }, isMac)}`,
		},
		{
			label: t("keyboardShortcuts.shortcut.copy", "Copy selected"),
			binding: formatBinding({ key: "c", ctrl: true }, isMac),
		},
		{
			label: t("keyboardShortcuts.shortcut.cut", "Cut selected"),
			binding: formatBinding({ key: "x", ctrl: true }, isMac),
		},
		{
			label: t("keyboardShortcuts.shortcut.paste", "Paste"),
			binding: formatBinding({ key: "v", ctrl: true }, isMac),
		},
		{
			label: t("keyboardShortcuts.shortcut.delete", "Delete selected"),
			binding: "Del / ⌫",
		},
		{
			label: t("keyboardShortcuts.shortcut.selectAllZooms", "Select all zoom regions"),
			binding: formatBinding({ key: "a", ctrl: true }, isMac),
		},
	];

	useEffect(() => {
		Promise.all([formatShortcut(["shift", "Scroll"]), formatShortcut(["mod", "Scroll"])]).then(
			([pan, zoom]) => setScrollLabels({ pan, zoom }),
		);
	}, []);

	return (
		<Dialog>
			<DialogTrigger asChild>
				<Button
					variant="ghost"
					size="sm"
					className={triggerClassName ?? APP_HEADER_ICON_BUTTON_CLASS}
					title={t("keyboardShortcuts.trigger", "Shortcuts")}
					aria-label={t("keyboardShortcuts.trigger", "Shortcuts")}
				>
					<Keyboard className="h-3.5 w-3.5" />
					{triggerLabel ? <span className="font-medium">{triggerLabel}</span> : null}
				</Button>
			</DialogTrigger>
			<DialogContent className="max-w-lg">
				<DialogHeader>
					<DialogTitle className="text-xl font-semibold text-foreground flex items-center gap-2">
						<Keyboard className="h-5 w-5 text-[#2563EB]" />{" "}
						{t("keyboardShortcuts.title")}
					</DialogTitle>
					<DialogDescription className="text-muted-foreground">
						{t(
							"keyboardShortcuts.description",
							"Quick reference for the timeline and editor controls.",
						)}
					</DialogDescription>
				</DialogHeader>
				<div className="mt-4 max-h-[60vh] space-y-4 overflow-y-auto pr-2">
					<div className="rounded-xl border border-foreground/10 bg-foreground/[0.03] p-4 space-y-2 text-xs">
						<p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground/70">
							{t("keyboardShortcuts.configurable", "Configurable")}
						</p>
						{SHORTCUT_ACTIONS.map((action) => (
							<div
								key={action}
								className="flex items-center justify-between gap-3 rounded-lg border border-foreground/5 bg-foreground/5 px-3 py-2.5"
							>
								<span className="text-muted-foreground">
									{shortcutLabel(action)}
								</span>
								<kbd className="rounded border border-foreground/10 bg-foreground/10 px-2 py-1 font-mono text-[#2563EB]">
									{formatBinding(shortcuts[action], isMac)}
								</kbd>
							</div>
						))}
						<div className="grid grid-cols-1 gap-2 pt-2 sm:grid-cols-3">
							<div className="rounded-lg border border-foreground/5 bg-foreground/5 px-3 py-2.5">
								<p className="text-[11px] uppercase tracking-[0.18em] text-muted-foreground/70">
									{t("keyboardShortcuts.panTimeline")}
								</p>
								<kbd className="mt-2 inline-flex rounded border border-foreground/10 bg-foreground/10 px-2 py-1 font-mono text-[#2563EB]">
									{scrollLabels.pan}
								</kbd>
							</div>
							<div className="rounded-lg border border-foreground/5 bg-foreground/5 px-3 py-2.5">
								<p className="text-[11px] uppercase tracking-[0.18em] text-muted-foreground/70">
									{t("keyboardShortcuts.zoomTimeline")}
								</p>
								<kbd className="mt-2 inline-flex rounded border border-foreground/10 bg-foreground/10 px-2 py-1 font-mono text-[#2563EB]">
									{scrollLabels.zoom}
								</kbd>
							</div>
							<div className="rounded-lg border border-foreground/5 bg-foreground/5 px-3 py-2.5">
								<p className="text-[11px] uppercase tracking-[0.18em] text-muted-foreground/70">
									{t("keyboardShortcuts.cycleAnnotations")}
								</p>
								<kbd className="mt-2 inline-flex rounded border border-foreground/10 bg-foreground/10 px-2 py-1 font-mono text-[#2563EB]">
									{t("keyboardShortcuts.tab")}
								</kbd>
							</div>
						</div>
					</div>
					<div className="rounded-xl border border-foreground/10 bg-foreground/[0.03] p-4 space-y-2 text-xs">
						<p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground/70">
							{t("keyboardShortcuts.navigation", "Navigation")}
						</p>
						{navigationShortcuts.map(({ label, binding }) => (
							<div key={label} className="flex items-center justify-between gap-3 rounded-lg border border-foreground/5 bg-foreground/5 px-3 py-2.5">
								<span className="text-muted-foreground">{label}</span>
								<kbd className="rounded border border-foreground/10 bg-foreground/10 px-2 py-1 font-mono text-[#2563EB]">{binding}</kbd>
							</div>
						))}
					</div>
					<div className="rounded-xl border border-foreground/10 bg-foreground/[0.03] p-4 space-y-2 text-xs">
						<p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground/70">
							{t("keyboardShortcuts.editing", "Editing")}
						</p>
						{editingShortcuts.map(({ label, binding }) => (
							<div key={label} className="flex items-center justify-between gap-3 rounded-lg border border-foreground/5 bg-foreground/5 px-3 py-2.5">
								<span className="text-muted-foreground">{label}</span>
								<kbd className="rounded border border-foreground/10 bg-foreground/10 px-2 py-1 font-mono text-[#2563EB]">{binding}</kbd>
							</div>
						))}
					</div>
					<div className="flex justify-end">
						<Button type="button" variant="outline" onClick={openConfig}>
							<Settings2 className="h-4 w-4" />
							{t("keyboardShortcuts.customize")}
						</Button>
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}

export function TutorialHelp() {
	const t = useScopedT("editor");

	return (
		<Dialog>
			<DialogTrigger asChild>
				<Button variant="ghost" size="sm" className={APP_HEADER_ACTION_BUTTON_CLASS}>
					<HelpCircle className="w-3.5 h-3.5" />
					<span className="font-medium">{t("tutorial.howTrimmingWorks")}</span>
				</Button>
			</DialogTrigger>
			<DialogContent className="max-w-2xl">
				<DialogHeader>
					<DialogTitle className="text-xl font-semibold text-foreground flex items-center gap-2">
						<Scissors className="w-5 h-5 text-[#ef4444]" /> {t("tutorial.title")}
					</DialogTitle>
					<DialogDescription className="text-muted-foreground">
						{t("tutorial.understanding")}
					</DialogDescription>
				</DialogHeader>
				<div className="mt-4 space-y-8">
					{/* Explanation */}
					<div className="bg-foreground/5 rounded-lg p-4 border border-foreground/5">
						<p className="text-muted-foreground leading-relaxed">
							{t("tutorial.descriptionP1")}
							<span className="text-[#ef4444] font-bold">
								{" "}
								{t("tutorial.descriptionRemove")}
							</span>
							. {t("tutorial.descriptionP3")}
						</p>
					</div>
					{/* Visual Illustration */}
					<div className="space-y-2">
						<h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wider">
							{t("tutorial.visualExample")}
						</h3>
						<div className="relative h-24 bg-[#000] rounded-lg border border-foreground/10 flex items-center px-4 overflow-hidden select-none">
							{/* Background track (Kept parts) */}
							<div className="absolute inset-x-4 h-2 bg-slate-600 rounded-full overflow-hidden">
								{/* Solid line representing video */}
							</div>
							{/* Removed Segment 1 */}
							<div
								className="absolute left-[20%] h-8 bg-[#ef4444]/20 border border-[#ef4444] rounded flex flex-col items-center justify-center z-10"
								style={{ width: "20%" }}
							>
								<span className="text-[10px] font-bold text-[#ef4444] bg-black/50 px-1 rounded">
									{t("tutorial.removed")}
								</span>
							</div>
							{/* Removed Segment 2 */}
							<div
								className="absolute left-[65%] h-8 bg-[#ef4444]/20 border border-[#ef4444] rounded flex flex-col items-center justify-center z-10"
								style={{ width: "15%" }}
							>
								<span className="text-[10px] font-bold text-[#ef4444] bg-black/50 px-1 rounded">
									{t("tutorial.removed")}
								</span>
							</div>
							{/* Labels for kept parts */}
							<div className="absolute left-[5%] text-[10px] text-muted-foreground font-medium">
								{t("tutorial.kept")}
							</div>
							<div className="absolute left-[50%] text-[10px] text-muted-foreground font-medium">
								{t("tutorial.kept")}
							</div>
							<div className="absolute left-[90%] text-[10px] text-muted-foreground font-medium">
								{t("tutorial.kept")}
							</div>
						</div>
						<div className="flex justify-center mt-2">
							<ArrowRight className="w-4 h-4 text-muted-foreground rotate-90" />
						</div>
						{/* Result */}
						<div className="relative h-12 bg-[#000] rounded-lg border border-foreground/10 flex items-center justify-center gap-1 px-4 select-none">
							<div
								className="h-8 bg-slate-700 rounded flex items-center justify-center opacity-80"
								style={{ width: "30%" }}
							>
								<span className="text-[10px] text-white font-medium">
									{t("tutorial.part", undefined, { number: "1" })}
								</span>
							</div>
							<div
								className="h-8 bg-slate-700 rounded flex items-center justify-center opacity-80"
								style={{ width: "30%" }}
							>
								<span className="text-[10px] text-white font-medium">
									{t("tutorial.part", undefined, { number: "2" })}
								</span>
							</div>
							<div
								className="h-8 bg-slate-700 rounded flex items-center justify-center opacity-80"
								style={{ width: "30%" }}
							>
								<span className="text-[10px] text-white font-medium">
									{t("tutorial.part", undefined, { number: "3" })}
								</span>
							</div>
							<span className="absolute right-4 text-xs text-muted-foreground">
								{t("tutorial.finalVideo")}
							</span>
						</div>
					</div>
					{/* Steps */}
					<div className="grid grid-cols-2 gap-4">
						<div className="p-3 rounded bg-foreground/5 border border-foreground/5">
							<div className="text-[#ef4444] font-bold mb-1">
								{t("tutorial.addTrimStep")}
							</div>
							<p className="text-xs text-muted-foreground">
								{t("tutorial.addTrimDesc")}
							</p>
						</div>
						<div className="p-3 rounded bg-foreground/5 border border-foreground/5">
							<div className="text-[#ef4444] font-bold mb-1">
								{t("tutorial.adjustStep")}
							</div>
							<p className="text-xs text-muted-foreground">
								{t("tutorial.adjustDesc")}
							</p>
						</div>
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}
