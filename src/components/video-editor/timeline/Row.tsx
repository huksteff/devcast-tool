import type { RowDefinition } from "dnd-timeline";
import { useRow } from "dnd-timeline";
import { TIMELINE_CLIP_ROW_HEIGHT_PX, TIMELINE_ROW_MIN_HEIGHT_PX } from "./timelineLayout";

interface RowProps extends RowDefinition {
	embedded?: boolean;
	caption?: boolean;
	compact?: boolean;
	filmstrip?: boolean;
	children: React.ReactNode;
	label?: string;
	hint?: string;
	isEmpty?: boolean;
	labelColor?: string;
	onDelete?: () => void;
	onMouseEnter?: React.MouseEventHandler<HTMLDivElement>;
	onMouseMove?: React.MouseEventHandler<HTMLDivElement>;
	onMouseLeave?: React.MouseEventHandler<HTMLDivElement>;
	onMouseDown?: React.MouseEventHandler<HTMLDivElement>;
	onClick?: React.MouseEventHandler<HTMLDivElement>;
}

export default function Row({
	id,
	embedded = false,
	compact = true,
	filmstrip = false,
	children,
	label,
	hint,
	isEmpty,
	labelColor = "#666",
	onDelete,
	onMouseEnter,
	onMouseMove,
	onMouseLeave,
	onMouseDown,
	onClick,
}: RowProps) {
	const { setNodeRef, rowWrapperStyle, rowStyle } = useRow({ id });

	return (
		<div
			data-timeline-row={id}
			className="bg-foreground/[0.018] border-t border-foreground/[0.10] relative"
			style={{
				...rowWrapperStyle,
				...(embedded
					? {
							position: "absolute" as const,
							insetInline: 0,
							bottom: 7,
							zIndex: 15,
							pointerEvents: "none" as const,
						}
					: {}),
				marginBottom: 2,
				flexGrow: 0,
				flexShrink: 0,
				height:
					embedded
						? 20
						: filmstrip || !compact
							? `calc(${TIMELINE_CLIP_ROW_HEIGHT_PX}px * var(--timeline-row-scale, 1))`
							: `calc(${TIMELINE_ROW_MIN_HEIGHT_PX}px * var(--timeline-row-scale, 1))`,
				flexBasis: embedded
					? 20
					: filmstrip || !compact
						? `calc(${TIMELINE_CLIP_ROW_HEIGHT_PX}px * var(--timeline-row-scale, 1))`
						: `calc(${TIMELINE_ROW_MIN_HEIGHT_PX}px * var(--timeline-row-scale, 1))`,
			}}
		>
			{label && (
				<div
					className="absolute left-1.5 top-1/2 -translate-y-1/2 text-[9px] font-semibold uppercase tracking-widest z-20 pointer-events-none select-none"
					style={{ color: labelColor, writingMode: "horizontal-tb" }}
				>
					{label}
				</div>
			)}
			{onDelete && <button type="button" aria-label={`Delete ${label ?? "annotation"} track`} title={`Delete ${label ?? "annotation"} track`} className="absolute right-2 top-1/2 z-40 -translate-y-1/2 rounded px-1.5 py-0.5 text-xs text-muted-foreground hover:bg-destructive/20 hover:text-destructive" onMouseDown={(event) => event.stopPropagation()} onClick={(event) => { event.stopPropagation(); onDelete(); }}>×</button>}
			{isEmpty && hint && (
				<div className="absolute inset-0 flex items-center justify-center pointer-events-none select-none z-10">
					<span className="text-[11px] text-foreground/15 font-medium">{hint}</span>
				</div>
			)}
			<div
				ref={setNodeRef}
				className="relative min-w-0 self-stretch"
				style={rowStyle}
				onMouseEnter={onMouseEnter}
				onMouseMove={onMouseMove}
				onMouseLeave={onMouseLeave}
				onMouseDown={onMouseDown}
				onClick={onClick}
			>
				{children}
			</div>
		</div>
	);
}
