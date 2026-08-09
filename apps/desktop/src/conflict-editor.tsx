import { useEffect, useMemo, useState } from "react";
import { t } from "./i18n";
import type { ConflictFileDto } from "./repository";

type ConflictHunk = Extract<
  ConflictFileDto["segments"][number],
  { kind: "conflict" }
>;

export function ConflictEditor({
  file,
  disabled,
  onApply,
}: {
  file: ConflictFileDto;
  disabled: boolean;
  onApply: (content: string) => Promise<boolean>;
}) {
  const hunks = useMemo(
    () =>
      file.segments.filter(
        (segment): segment is ConflictHunk => segment.kind === "conflict",
      ),
    [file.segments],
  );
  const [results, setResults] = useState<Map<number, string>>(new Map());
  const [applying, setApplying] = useState(false);

  useEffect(() => {
    setResults(new Map());
  }, [file.worktreeOid]);

  function setResult(index: number, content: string) {
    setResults((current) => {
      const next = new Map(current);
      next.set(index, content);
      return next;
    });
  }

  const resolvedCount = hunks.filter((hunk) => results.has(hunk.index)).length;
  const resultContent = file.segments
    .map((segment) =>
      segment.kind === "common"
        ? segment.content
        : (results.get(segment.index) ?? ""),
    )
    .join("");
  const ready = file.editable && resolvedCount === hunks.length && hunks.length > 0;

  async function apply() {
    if (!ready || applying) return;
    setApplying(true);
    try {
      await onApply(resultContent);
    } finally {
      setApplying(false);
    }
  }

  if (!file.editable) {
    return (
      <div className="conflict-panel" role="region" aria-label={t("Conflict resolution guidance")}>
        <h2>{t("Built-in merge editor unavailable")}</h2>
        <p>{file.unavailableReason ?? t("Use a whole-file resolution action for this conflict.")}</p>
      </div>
    );
  }

  return (
    <div className="conflict-editor" role="region" aria-label={t("Three-way merge editor")}>
      <div className="conflict-editor-heading">
        <div>
          <span className="eyebrow">{t("Three-way merge editor")}</span>
          <strong>{t("{resolved} of {total} hunks resolved", {
            resolved: resolvedCount,
            total: hunks.length,
          })}</strong>
        </div>
        <button
          type="button"
          disabled={!ready || disabled || applying}
          onClick={() => void apply()}
        >
          {applying ? t("Applying resolved file…") : t("Apply resolved file")}
        </button>
      </div>

      <div className="conflict-source-grid" aria-label={t("Conflict source versions")}>
        <ConflictSource title={t("Base")} content={file.base} />
        <ConflictSource title={t("Current")} content={file.ours} />
        <ConflictSource title={t("Incoming")} content={file.theirs} />
      </div>

      <div className="conflict-hunk-list">
        {hunks.map((hunk) => {
          const resolved = results.has(hunk.index);
          return (
            <section
              className={resolved ? "conflict-hunk resolved" : "conflict-hunk"}
              key={hunk.index}
              aria-label={t("Conflict hunk {number}", { number: hunk.index + 1 })}
            >
              <div className="conflict-hunk-heading">
                <div>
                  <span>{t("Hunk {number}", { number: hunk.index + 1 })}</span>
                  <small>{resolved ? t("Resolved") : t("Choose or edit a result")}</small>
                </div>
                <div className="conflict-hunk-actions">
                  <button type="button" disabled={disabled} onClick={() => setResult(hunk.index, hunk.ours)}>
                    {t("Use current")}
                  </button>
                  <button type="button" disabled={disabled} onClick={() => setResult(hunk.index, hunk.theirs)}>
                    {t("Use incoming")}
                  </button>
                  <button type="button" disabled={disabled} onClick={() => setResult(hunk.index, hunk.ours + hunk.theirs)}>
                    {t("Use both")}
                  </button>
                </div>
              </div>
              <div className="conflict-hunk-sides">
                <ConflictSource title={t("Current")} content={hunk.ours} />
                <ConflictSource title={t("Base")} content={hunk.base} />
                <ConflictSource title={t("Incoming")} content={hunk.theirs} />
              </div>
              <label className="conflict-result">
                <span>{t("Resolved result")}</span>
                <textarea
                  aria-label={t("Resolved result for hunk {number}", { number: hunk.index + 1 })}
                  disabled={disabled}
                  spellCheck={false}
                  value={results.get(hunk.index) ?? ""}
                  placeholder={t("Choose a side or type the resolved content")}
                  onChange={(event) => setResult(hunk.index, event.currentTarget.value)}
                />
              </label>
            </section>
          );
        })}
      </div>
    </div>
  );
}

function ConflictSource({
  title,
  content,
}: {
  title: string;
  content?: string;
}) {
  return (
    <div className="conflict-source">
      <span>{title}</span>
      <pre>{content ?? t("Not present")}</pre>
    </div>
  );
}