/**
 * SpreadsheetEditor — editable spreadsheet with sheet tabs,
 * inline cell editing, add/remove rows/columns/sheets.
 * Matches Leptos spreadsheet_editor.rs.
 */
import { useState, useCallback, useRef } from "react";
import type { DocData, SheetData, OpenFileEntry } from "../types";

interface Props {
  path: string;
  docData: DocData;
  setOpenFiles: React.Dispatch<React.SetStateAction<OpenFileEntry[]>>;
}

function colIndexToLetter(idx: number): string {
  let result = "";
  let n = idx;
  for (;;) {
    result = String.fromCharCode(65 + (n % 26)) + result;
    if (n < 26) break;
    n = Math.floor(n / 26) - 1;
  }
  return result;
}

export function SpreadsheetEditor({ path, docData, setOpenFiles }: Props) {
  if (docData.type !== "spreadsheet") {
    return <div className="spreadsheet-empty">Not a spreadsheet</div>;
  }

  const [activeTab, setActiveTab] = useState(0);
  const [sheets, setSheets] = useState<SheetData[]>(docData.sheets);

  const syncEdits = useCallback((newSheets: SheetData[]) => {
    setSheets(newSheets);
    setOpenFiles((prev) =>
      prev.map((f) =>
        f.path === path
          ? { ...f, editedDocData: { type: "spreadsheet", sheets: newSheets } }
          : f,
      ),
    );
  }, [path, setOpenFiles]);

  const addSheet = useCallback(() => {
    const n = sheets.length + 1;
    const next = [...sheets, { name: `Sheet${n}`, rows: [[""]] }];
    setActiveTab(next.length - 1);
    syncEdits(next);
  }, [sheets, syncEdits]);

  const removeSheet = useCallback((idx: number) => {
    if (sheets.length <= 1) return;
    const next = sheets.filter((_, i) => i !== idx);
    const newActive = Math.min(idx, next.length - 1);
    setActiveTab(newActive);
    syncEdits(next);
  }, [sheets, syncEdits]);

  if (sheets.length === 0) {
    return <div className="spreadsheet-empty">Empty spreadsheet</div>;
  }

  const sheet = sheets[activeTab] ?? sheets[0];

  return (
    <div className="spreadsheet-viewer">
      <div className="spreadsheet-tabs">
        {sheets.map((s, i) => (
          <div key={i} className={`spreadsheet-tab-group${activeTab === i ? " active" : ""}`}>
            <button className="spreadsheet-tab-label" onClick={() => setActiveTab(i)}>{s.name}</button>
            <button className="spreadsheet-tab-close" title="Remove sheet" onClick={() => removeSheet(i)}>&times;</button>
          </div>
        ))}
        <button className="spreadsheet-tab spreadsheet-tab-add" title="Add sheet" onClick={addSheet}>+</button>
      </div>
      <EditableSheet
        sheetIdx={activeTab}
        sheet={sheet}
        sheets={sheets}
        syncEdits={syncEdits}
      />
    </div>
  );
}

interface EditableSheetProps {
  sheetIdx: number;
  sheet: SheetData;
  sheets: SheetData[];
  syncEdits: (s: SheetData[]) => void;
}

function EditableSheet({ sheetIdx, sheet, sheets, syncEdits }: EditableSheetProps) {
  const maxCols = Math.max(1, ...sheet.rows.map((r) => r.length));
  const sheetsRef = useRef(sheets);
  sheetsRef.current = sheets;

  const addRow = useCallback(() => {
    const all = sheetsRef.current.map((s, i) => {
      if (i !== sheetIdx) return s;
      const cols = Math.max(1, ...s.rows.map((r) => r.length));
      return { ...s, rows: [...s.rows, Array(cols).fill("")] };
    });
    syncEdits(all);
  }, [sheetIdx, syncEdits]);

  const addCol = useCallback(() => {
    const all = sheetsRef.current.map((s, i) => {
      if (i !== sheetIdx) return s;
      return { ...s, rows: s.rows.map((row) => [...row, ""]) };
    });
    syncEdits(all);
  }, [sheetIdx, syncEdits]);

  const rmLastRow = useCallback(() => {
    const all = sheetsRef.current.map((s, i) => {
      if (i !== sheetIdx || s.rows.length <= 1) return s;
      return { ...s, rows: s.rows.slice(0, -1) };
    });
    syncEdits(all);
  }, [sheetIdx, syncEdits]);

  const rmLastCol = useCallback(() => {
    const all = sheetsRef.current.map((s, i) => {
      if (i !== sheetIdx) return s;
      return { ...s, rows: s.rows.map((row) => row.length > 1 ? row.slice(0, -1) : row) };
    });
    syncEdits(all);
  }, [sheetIdx, syncEdits]);

  const handleCellBlur = useCallback((rIdx: number, cIdx: number, el: HTMLTableCellElement) => {
    const newVal = el.innerText;
    const all = [...sheetsRef.current];
    const s = { ...all[sheetIdx], rows: all[sheetIdx].rows.map((r) => [...r]) };
    while (s.rows.length <= rIdx) s.rows.push(Array(maxCols).fill(""));
    while (s.rows[rIdx].length <= cIdx) s.rows[rIdx].push("");
    if (s.rows[rIdx][cIdx] !== newVal) {
      s.rows[rIdx][cIdx] = newVal;
      all[sheetIdx] = s;
      syncEdits(all);
    }
  }, [sheetIdx, maxCols, syncEdits]);

  return (
    <>
      <div className="spreadsheet-toolbar">
        <button className="spreadsheet-action-btn" onClick={addRow} title="Add row">+Row</button>
        <button className="spreadsheet-action-btn" onClick={addCol} title="Add column">+Col</button>
        <button className="spreadsheet-action-btn spreadsheet-action-danger" onClick={rmLastRow} title="Remove last row">-Row</button>
        <button className="spreadsheet-action-btn spreadsheet-action-danger" onClick={rmLastCol} title="Remove last column">-Col</button>
      </div>
      <div className="spreadsheet-table-wrap">
        <table>
          <thead>
            <tr>
              <th className="spreadsheet-row-num">#</th>
              {Array.from({ length: maxCols }, (_, i) => (
                <th key={i}>{colIndexToLetter(i)}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {sheet.rows.map((row, rIdx) => (
              <tr key={rIdx}>
                <td className="spreadsheet-row-num">{rIdx + 1}</td>
                {Array.from({ length: maxCols }, (_, cIdx) => {
                  const value = row[cIdx] ?? "";
                  const isNum = value !== "" && !isNaN(Number(value));
                  return (
                    <td
                      key={cIdx}
                      className={isNum ? "spreadsheet-cell-num spreadsheet-cell-edit" : "spreadsheet-cell-edit"}
                      contentEditable
                      suppressContentEditableWarning
                      onBlur={(e) => handleCellBlur(rIdx, cIdx, e.currentTarget)}
                    >{value}</td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
