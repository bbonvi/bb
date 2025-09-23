import { useEffect, useMemo, useRef, useState } from 'react';
import { Bmark } from '../api';

interface UseGridLayoutProps {
    bmarks: Bmark[];
    shuffleSeed: number;
    shuffle: boolean;
}

let DEFAULT_COLUMNS = 4;
const innerWidth = window.innerWidth - 10;

if (innerWidth < 768) {
    DEFAULT_COLUMNS = 1;
} else if (innerWidth < 1024) {
    DEFAULT_COLUMNS = 2;
} else if (innerWidth < 1280) {
    DEFAULT_COLUMNS = 3;
}

let MIN_ROW_HEIGHT = 400;
if (DEFAULT_COLUMNS <= 2) {
    MIN_ROW_HEIGHT = 525;
}

// Deterministic shuffle
function shuffleArray(arr: any[], seed: number = 1) {
    const array = [...arr];
    let random = function() {
        var x = Math.sin(seed++) * 10000;
        return x - Math.floor(x);
    };
    array.sort(() => random() - 0.5);
    return array;
}

export function useGridLayout({ bmarks, shuffleSeed, shuffle }: UseGridLayoutProps) {
    const [columns, setColumns] = useState(DEFAULT_COLUMNS);
    const [renderKey, _setRenderKey] = useState(0);
    const refreshTimerRef = useRef({ "1": 0, "2": 0, "3": 0 });

    const bmarksShuffled = useMemo(() => {
        let bookmarks = bmarks;
        if (shuffle) {
            bookmarks = shuffleArray(bookmarks, shuffleSeed);
        }
        return bookmarks;
    }, [bmarks, shuffleSeed, shuffle]);

    const setCols = () => {
        const innerWidth = window.innerWidth;

        let cols = DEFAULT_COLUMNS;
        if (innerWidth < 768) {
            cols = 1;
        } else if (innerWidth < 1024) {
            cols = 2;
        } else if (innerWidth < 1280) {
            cols = 3;
        } else if (innerWidth < 1920) {
            cols = 4;
        } else {
            cols = 5;
        }

        MIN_ROW_HEIGHT = 400;
        if (cols <= 2) {
            MIN_ROW_HEIGHT = 525;
        }

        setColumns(cols);
        return cols;
    };

    const rerenderList = () => {
        _setRenderKey(Date.now());
    };

    useEffect(() => {
        const onResize = () => {
            cancelAnimationFrame(refreshTimerRef.current["3"]);
            refreshTimerRef.current["3"] = requestAnimationFrame(() => {
                setCols();
                rerenderList();
            }) as any;
        };

        window.addEventListener("resize", onResize);

        return () => {
            window.removeEventListener("resize", onResize);
        };
    }, []);

    return {
        columns,
        setColumns,
        renderKey,
        rerenderList,
        bmarksShuffled,
        setCols,
        MIN_ROW_HEIGHT,
    };
}