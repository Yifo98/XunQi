import { useState } from "react";
import {
  CaretDownIcon,
  CaretRightIcon,
  CaretLeftIcon,
  FileTextIcon,
  FunnelSimpleIcon,
  MagnifyingGlassIcon,
  TrashIcon,
  VideoCameraIcon,
} from "@phosphor-icons/react";
import type { CaptureKind, CaptureTaskDetail } from "../lib/backend";

export type KindFilter = "all" | CaptureKind;

type TaskGroup = {
  sourceName: string;
  tasks: CaptureTaskDetail[];
};

type TaskSidebarProps = {
  groups: TaskGroup[];
  totalCount: number;
  articleCount: number;
  videoCount: number;
  query: string;
  filter: KindFilter;
  activeTaskId: number | null;
  selectedTaskIds: Set<number>;
  collapsedSources: Set<string>;
  selectedCount: number;
  selectedArticleCount: number;
  selectedVideoCount: number;
  visibleTaskCount: number;
  allVisibleSelected: boolean;
  batchBusy: boolean;
  onQueryChange: (value: string) => void;
  onFilterChange: (filter: KindFilter) => void;
  onActivate: (taskId: number) => void;
  onToggleSelected: (taskId: number) => void;
  onToggleVisibleSelection: () => void;
  onClearSelection: () => void;
  onToggleSource: (sourceName: string) => void;
  onClearSelected: () => void;
  onClearCompleted: () => void;
  onCollapse: () => void;
};

export function TaskSidebar({
  groups,
  totalCount,
  articleCount,
  videoCount,
  query,
  filter,
  activeTaskId,
  selectedTaskIds,
  collapsedSources,
  selectedCount,
  selectedArticleCount,
  selectedVideoCount,
  visibleTaskCount,
  allVisibleSelected,
  batchBusy,
  onQueryChange,
  onFilterChange,
  onActivate,
  onToggleSelected,
  onToggleVisibleSelection,
  onClearSelection,
  onToggleSource,
  onClearSelected,
  onClearCompleted,
  onCollapse,
}: TaskSidebarProps) {
  const [expandedSources, setExpandedSources] = useState<Set<string>>(new Set());
  const visibleSelectedCount = groups.reduce(
    (count, group) => count + group.tasks.filter(({ task }) => selectedTaskIds.has(task.id)).length,
    0,
  );
  const hasHiddenSelection = selectedCount > visibleSelectedCount;

  return (
    <aside className="task-sidebar" aria-label="捕获任务">
      <div className="sidebar-heading">
        <div>
          <strong>捕获任务</strong>
          <span>{totalCount}</span>
        </div>
        <button type="button" className="icon-button" onClick={onCollapse} aria-label="收起任务列表">
          <CaretLeftIcon size={20} weight="bold" />
        </button>
      </div>

      <div className="task-search-row">
        <label className="task-search">
          <MagnifyingGlassIcon size={18} />
          <input
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="搜索公众号、视频号或标题"
            aria-label="搜索公众号、视频号或标题"
          />
        </label>
        <span className="filter-icon" aria-hidden="true" title="使用下方标签筛选">
          <FunnelSimpleIcon size={19} />
        </span>
      </div>

      <div className="kind-tabs" role="tablist" aria-label="任务类型">
        <FilterTab selected={filter === "all"} onClick={() => onFilterChange("all")}>
          全部 <span>{totalCount}</span>
        </FilterTab>
        <FilterTab selected={filter === "article"} onClick={() => onFilterChange("article")}>
          公众号 <span>{articleCount}</span>
        </FilterTab>
        <FilterTab selected={filter === "video"} onClick={() => onFilterChange("video")}>
          视频号 <span>{videoCount}</span>
        </FilterTab>
      </div>

      {visibleTaskCount > 0 && (
        <div className="bulk-selection-row">
          <button
            type="button"
            className={allVisibleSelected ? "bulk-selection-active" : ""}
            aria-pressed={allVisibleSelected}
            onClick={onToggleVisibleSelection}
            disabled={batchBusy}
          >
            <span className="bulk-selection-mark" aria-hidden="true">{allVisibleSelected ? "✓" : ""}</span>
            {allVisibleSelected
              ? `${hasHiddenSelection ? "取消当前" : "取消全选"}${filterLabel(filter)} ${visibleTaskCount} 项`
              : `全选${filterLabel(filter)} ${visibleTaskCount} 项`}
          </button>
          {hasHiddenSelection ? (
            <button
              type="button"
              className="bulk-clear-all"
              onClick={onClearSelection}
              disabled={batchBusy}
            >
              取消全部已选 {selectedCount} 项
            </button>
          ) : (
            <small>切换分类后仍保留已选项</small>
          )}
        </div>
      )}

      <div className="task-groups">
        {groups.length === 0 ? (
          <div className="sidebar-empty">
            <MagnifyingGlassIcon size={30} />
            <strong>{totalCount === 0 ? "还没有捕获任务" : "没有匹配的任务"}</strong>
            <p>{totalCount === 0 ? "去微信复制文章或视频号分享链接。" : "换个关键词或清除筛选。"}</p>
          </div>
        ) : (
          groups.map((group) => {
            const collapsed = collapsedSources.has(group.sourceName);
            const expanded = expandedSources.has(group.sourceName);
            const visibleTasks = expanded ? group.tasks : group.tasks.slice(0, 3);
            return (
              <section className="task-group" key={group.sourceName}>
                <button
                  type="button"
                  className="task-group-heading"
                  onClick={() => onToggleSource(group.sourceName)}
                  aria-expanded={!collapsed}
                >
                  <span className="group-caret">
                    {collapsed ? <CaretRightIcon size={15} /> : <CaretDownIcon size={15} />}
                  </span>
                  <strong>{group.sourceName}</strong>
                  <span>{group.tasks.length}</span>
                </button>
                {!collapsed && (
                  <div className="task-rows">
                    {visibleTasks.map(({ task }) => (
                      <div
                        className={`task-row ${activeTaskId === task.id ? "task-row-active" : ""}`}
                        key={task.id}
                      >
                        <label className="task-checkbox" onClick={(event) => event.stopPropagation()}>
                          <input
                            type="checkbox"
                            checked={selectedTaskIds.has(task.id)}
                            onChange={() => onToggleSelected(task.id)}
                            aria-label={`选择${task.title}`}
                          />
                        </label>
                        <button type="button" className="task-row-main" onClick={() => onActivate(task.id)}>
                          <span className="task-kind-icon" aria-hidden="true">
                            {task.kind === "article" ? (
                              <FileTextIcon size={20} />
                            ) : (
                              <VideoCameraIcon size={20} />
                            )}
                          </span>
                          <span className="task-row-copy">
                            <span className="task-title-line">
                              <strong>{task.title}</strong>
                              <time dateTime={task.createdAt}>{formatTaskTime(task.createdAt)}</time>
                            </span>
                            <TaskStatus status={task.status} statusDetail={task.statusDetail} />
                          </span>
                        </button>
                      </div>
                    ))}
                    {group.tasks.length > 3 && (
                      <button
                        type="button"
                        className="show-more-tasks"
                        onClick={() => setExpandedSources((current) => toggleSource(current, group.sourceName))}
                      >
                        {expanded ? "收起" : `查看更多 (${group.tasks.length - 3})`}
                        <CaretDownIcon className={expanded ? "show-more-open" : ""} size={14} />
                      </button>
                    )}
                  </div>
                )}
              </section>
            );
          })
        )}
      </div>

      <div className="sidebar-footer">
        {selectedCount > 0 ? (
          <div className="batch-bar">
            <div>
              <strong>已选 {selectedCount} 项</strong>
              <small>
                公众号 {selectedArticleCount}
                {selectedVideoCount > 0 ? ` · 视频号 ${selectedVideoCount}` : ""}
              </small>
            </div>
            <div className="batch-actions">
              <button type="button" className="remove-selected" onClick={onClearSelected} disabled={batchBusy}>
                <TrashIcon size={16} />
                移除并清缓存
              </button>
            </div>
          </div>
        ) : (
          <button type="button" className="clear-completed" onClick={onClearCompleted}>
            <TrashIcon size={17} />
            清空已完成缓存
          </button>
        )}
        <p>内容留在本地 · A QIDU Utility</p>
      </div>
    </aside>
  );
}

function filterLabel(filter: KindFilter) {
  return {
    all: "全部",
    article: "公众号",
    video: "视频号",
  }[filter];
}

function toggleSource(values: Set<string>, sourceName: string) {
  const next = new Set(values);
  if (next.has(sourceName)) next.delete(sourceName);
  else next.add(sourceName);
  return next;
}

function FilterTab({
  selected,
  onClick,
  children,
}: {
  selected: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button type="button" role="tab" aria-selected={selected} className={selected ? "active" : ""} onClick={onClick}>
      {children}
    </button>
  );
}

function TaskStatus({ status, statusDetail }: Pick<CaptureTaskDetail["task"], "status" | "statusDetail">) {
  const label = {
    queued: "等待处理",
    processing: statusDetail.includes("视频") ? "正在识别" : "正在读取",
    ready: statusDetail.includes("视频") ? "可下载" : "可导出",
    needs_attention: "需查看",
    exporting: "正在导出",
    downloading: "下载中",
    completed: "已完成",
    failed: "失败",
  }[status];
  return <span className={`row-status row-status-${status}`}>{label}</span>;
}

function formatTaskTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "刚刚";
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}
