import { Text, Tooltip } from "@mantine/core";
import type { ReactNode } from "react";

export interface BreakdownRow {
  label: ReactNode;
  value: ReactNode;
  hint?: string;
  total?: boolean;
  dim?: boolean;
  tone?: "good" | "bad" | "warn";
}

/** Label/amount list used for every cost and price breakdown. */
export function Breakdown({ rows }: { rows: BreakdownRow[] }) {
  return (
    <div role="table">
      {rows.map((r, i) => (
        <div role="row" key={i} className={`srm-breakdown-row${r.total ? " srm-breakdown-total" : ""}`}>
          <Text role="cell" size="sm" c={r.dim ? "dimmed" : undefined} fw={r.total ? 650 : undefined}>
            {r.hint ? (
              <Tooltip label={<span style={{ whiteSpace: "pre-line" }}>{r.hint}</span>} multiline w={300}>
                <span style={{ borderBottom: "1px dotted currentColor", cursor: "help" }}>{r.label}</span>
              </Tooltip>
            ) : (
              r.label
            )}
          </Text>
          <Text
            role="cell"
            size="sm"
            className="srm-num"
            fw={r.total ? 650 : undefined}
            c={r.tone ? `var(--srm-${r.tone})` : r.dim ? "dimmed" : undefined}
          >
            {r.value}
          </Text>
        </div>
      ))}
    </div>
  );
}
