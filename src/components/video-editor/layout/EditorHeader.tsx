import { Input } from "@/components/ui/input";
import {
	FolderOpen,
	FilmStrip,
	ArrowClockwise as Redo2,
	ArrowCounterClockwise as Undo2,
} from "@phosphor-icons/react";
import type { CSSProperties, FormEvent, RefObject } from "react";
import { Button } from "@/components/ui/button";
import type { useI18n } from "@/contexts/I18nContext";
import type { useExportDimensions } from "../export/useExportDimensions";
import type { useExportSession } from "../export/useExportSession";
import type { useExportSettings } from "../export/useExportSettings";
import type { useExportStatusViewModel } from "../export/useExportStatusViewModel";
import type { useVideoEditorPresets } from "../presets/useVideoEditorPresets";
import type { useProjectState } from "../state/useProjectState";
import { APP_HEADER_ICON_BUTTON_CLASS, DiscordLinkButton, FeedbackDialog } from "../TutorialHelp";
import { EditorExportMenu } from "./EditorExportMenu";
import { EditorPresetMenu } from "./EditorPresetMenu";

type Props = {
	videosOpen?: boolean;
	onToggleVideos?: () => void;
	t: ReturnType<typeof useI18n>["t"];
	headerLeftControlsPaddingClass: string;
	project: ReturnType<typeof useProjectState>;
	projectBrowserTriggerRef: RefObject<HTMLButtonElement | null>;
	projectNameInputRef: RefObject<HTMLInputElement | null>;
	projectDisplayName: string;
	hasUnsavedChanges: boolean;
	canUndo: boolean;
	canRedo: boolean;
	handleOpenProjectBrowser: () => void;
	handleUndo: () => void;
	handleRedo: () => void;
	handleProjectNameSubmit: (event?: FormEvent<HTMLFormElement>) => void;
	closeProjectNameEditor: () => void;
	presets: ReturnType<typeof useVideoEditorPresets>;
	exportSettings: ReturnType<typeof useExportSettings>;
	exportSession: ReturnType<typeof useExportSession>;
	exportDimensions: ReturnType<typeof useExportDimensions>;
	exportStatus: ReturnType<typeof useExportStatusViewModel>;
	hasCaptionsForSidecar: boolean;
	nvidiaCudaExportAvailable: boolean;
	experimentalNvidiaCudaExport: boolean;
	setExperimentalNvidiaCudaExport: (enabled: boolean) => void;
	handleOpenExportDropdown: () => void;
	handleExportDropdownClose: () => void;
	handleCancelExport: () => void;
	handleRetrySaveExport: () => void;
	handleStartExportFromDropdown: () => void;
	revealExportedFile: () => void;
	exportMessage: string | null;
	prepareExportForShare: () => Promise<string | undefined>;
	onRequestShareSignIn: () => void;
	shareRequestNonce: number;
	authToken?: string;
};

export function EditorHeader(props: Props) {
	const {
		t,
		headerLeftControlsPaddingClass,
		project,
		projectBrowserTriggerRef,
		projectNameInputRef,
		projectDisplayName,
		hasUnsavedChanges,
		canUndo,
		canRedo,
		handleOpenProjectBrowser,
		handleUndo,
		handleRedo,
		handleProjectNameSubmit,
		closeProjectNameEditor,
		presets,
		exportSettings,
		exportSession,
		exportDimensions,
		exportStatus,
		hasCaptionsForSidecar,
		nvidiaCudaExportAvailable,
		experimentalNvidiaCudaExport,
		setExperimentalNvidiaCudaExport,
		handleOpenExportDropdown,
		handleExportDropdownClose,
		handleCancelExport,
		handleRetrySaveExport,
		handleStartExportFromDropdown,
		revealExportedFile,
		exportMessage,
	} = props;
	const {
		isEditingProjectName,
		setIsEditingProjectName,
		projectNameDraft,
		setProjectNameDraft,
		isSavingProjectName,
	} = project;

	return (
		<header
			className="editor-header [--text-sm:0.8125rem] relative z-50 grid h-14 shrink-0 border-b border-separator grid-cols-[minmax(0,1fr)_minmax(0,0.8fr)_minmax(0,1fr)] items-center gap-3 px-4"
			style={{ WebkitAppRegion: "drag" } as CSSProperties}
		>
			<div
				className={`editor-header-start flex items-center justify-self-start gap-1 ${headerLeftControlsPaddingClass}`}
				style={{ WebkitAppRegion: "no-drag" } as CSSProperties}
			>
				<Button
					type="button"
					variant="secondary"
					className="[--button-bg:var(--surface)] [--button-fg:var(--foreground)] mr-2 inline-flex h-9 min-w-[104px] items-center justify-center gap-2 px-4.5"
					aria-expanded={props.videosOpen}
					onClick={props.onToggleVideos}
				>
					<FilmStrip className="h-4 w-4" />
					<span className="text-sm font-semibold tracking-tight">
						{t("editor.library.videos", "Videos")}
					</span>
				</Button>
				<Button
					ref={projectBrowserTriggerRef}
					type="button"
					variant="ghost"
					size="sm"
					onClick={handleOpenProjectBrowser}
					className={APP_HEADER_ICON_BUTTON_CLASS}
					title={t("editor.project.projects", "Open projects")}
					aria-label={t("editor.project.projects", "Open projects")}
				>
					<FolderOpen className="h-4 w-4" />
				</Button>
				<div className="editor-header-community flex items-center gap-1">
					<DiscordLinkButton />
					<FeedbackDialog />
				</div>
				<div className="mx-2 h-4 w-px shrink-0 bg-separator" />
				<Button
					type="button"
					variant="ghost"
					onClick={handleUndo}
					disabled={!canUndo}
					className="inline-flex h-9 w-9 min-w-9 items-center justify-center p-0 disabled:cursor-not-allowed"
					title={t("common.actions.undo", "Undo")}
					aria-label={t("common.actions.undo", "Undo")}
				>
					<Undo2 className="h-4 w-4" />
				</Button>
				<Button
					type="button"
					variant="ghost"
					onClick={handleRedo}
					disabled={!canRedo}
					className="inline-flex h-9 w-9 min-w-9 items-center justify-center p-0 disabled:cursor-not-allowed"
					title={t("common.actions.redo", "Redo")}
					aria-label={t("common.actions.redo", "Redo")}
				>
					<Redo2 className="h-4 w-4" />
				</Button>
			</div>

			<div
				className="editor-header-title flex min-w-0 items-center justify-center"
				style={{ WebkitAppRegion: "no-drag" } as CSSProperties}
			>
				{isEditingProjectName ? (
					<form
						onSubmit={(event) => void handleProjectNameSubmit(event)}
						className="flex w-full min-w-0 items-center gap-1"
					>
						{hasUnsavedChanges ? (
							<span className="size-1.5 shrink-0 rounded-full bg-accent" />
						) : null}
						<Input
							ref={projectNameInputRef}
							type="text"
							value={projectNameDraft}
							onChange={(event) => setProjectNameDraft(event.target.value)}
							onBlur={() => {
								if (!isSavingProjectName) closeProjectNameEditor();
							}}
							onKeyDown={(event) => {
								if (event.key === "Escape") {
									event.preventDefault();
									closeProjectNameEditor();
								}
							}}
							disabled={isSavingProjectName}
							className="min-w-0 w-full text-sm disabled:cursor-wait"
							aria-label={t("editor.project.renameInput", "Project name")}
						/>
						<span className="project-file-extension shrink-0 text-xs font-medium tracking-tight text-muted-foreground/70">
							.recordly
						</span>
					</form>
				) : (
					<Button
						variant="ghost"
						type="button"
						onClick={() => setIsEditingProjectName(true)}
						className="inline-flex h-9 min-w-0 max-w-full items-center gap-1.5 px-3"
						title={t("editor.project.renameTitle", "Rename project")}
						aria-label={t("editor.project.renameTitle", "Rename project")}
					>
						{hasUnsavedChanges ? (
							<span className="size-1.5 shrink-0 rounded-full bg-accent" />
						) : null}
						<span className="truncate text-[13px] font-medium tracking-tight text-foreground/90">
							{projectDisplayName}
						</span>
						<span className="project-file-extension shrink-0 text-xs font-medium tracking-tight text-muted-foreground/70">
							.recordly
						</span>
					</Button>
				)}
			</div>

			<div
				className="editor-header-end flex min-w-0 items-center justify-self-end gap-3"
				style={{ WebkitAppRegion: "no-drag" } as CSSProperties}
			>
				<EditorPresetMenu t={t} presets={presets} />
				<EditorExportMenu
					t={t}
					exportSettings={exportSettings}
					exportSession={exportSession}
					exportDimensions={exportDimensions}
					exportStatus={exportStatus}
					hasCaptionsForSidecar={hasCaptionsForSidecar}
					nvidiaCudaExportAvailable={nvidiaCudaExportAvailable}
					experimentalNvidiaCudaExport={experimentalNvidiaCudaExport}
					setExperimentalNvidiaCudaExport={setExperimentalNvidiaCudaExport}
					handleOpenExportDropdown={handleOpenExportDropdown}
					handleExportDropdownClose={handleExportDropdownClose}
					handleCancelExport={handleCancelExport}
					handleRetrySaveExport={handleRetrySaveExport}
					handleStartExportFromDropdown={handleStartExportFromDropdown}
					revealExportedFile={revealExportedFile}
					exportMessage={exportMessage}
					projectTitle={projectDisplayName}
					prepareExportForShare={props.prepareExportForShare}
					onRequestShareSignIn={props.onRequestShareSignIn}
					shareRequestNonce={props.shareRequestNonce}
					authToken={props.authToken}
				/>
			</div>
		</header>
	);
}
