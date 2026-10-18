import React, { Suspense, lazy } from "react";
import { Loading } from "./Unavailable";

// three.js is large, so it only loads when an STL card is actually opened.
const StlScene = lazy(() => import("./StlScene"));

export interface StlViewerProps {
  readonly url: string;
  readonly href: string | null;
  readonly onMeta: (text: string) => void;
}

export function StlViewer(props: StlViewerProps) {
  return (
    <Suspense fallback={<Loading label="Loading 3D viewer…" />}>
      <StlScene {...props} />
    </Suspense>
  );
}
