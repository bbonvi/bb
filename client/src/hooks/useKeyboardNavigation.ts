import { useEffect } from 'react';
import { useDispatch, useSelector } from 'react-redux';
import { RootState } from '../store';
import * as globalSlice from '../store/globalSlice';
import { Bmark, UpdateBmark } from '../api';
import { isModKey, toBase64 } from '../helpers';
import { toast } from 'react-hot-toast';

interface UseKeyboardNavigationProps {
    bmarks: Bmark[];
    columns: number;
    showSettings: boolean;
    creating: boolean;
    editingId: number;
    setCreating: (val: boolean) => void;
    setEditingId: (id: number) => void;
    setPastedUrl: (url: string | undefined) => void;
    handleDelete: (id: number) => void;
    handleSave: (update: UpdateBmark) => void;
    containerRef: React.RefObject<HTMLDivElement>;
}

export function useKeyboardNavigation({
    bmarks,
    columns,
    showSettings,
    creating,
    editingId,
    setCreating,
    setEditingId,
    setPastedUrl,
    handleDelete,
    handleSave,
    containerRef,
}: UseKeyboardNavigationProps) {
    const dispatch = useDispatch();
    const focused = useSelector((state: RootState) => state.global.focusedIdx);
    const setFocused = (idx: number) => dispatch(globalSlice.setFocusedIdx(idx));

    useEffect(() => {
        const onKeyDown = (e: KeyboardEvent) => {
            if (showSettings) {
                return;
            }

            // Defocus inputs (but allow navigation with checkboxes)
            const activeEl = document.activeElement as HTMLInputElement;
            if (
                (activeEl?.tagName === "INPUT" && activeEl.type !== "checkbox")
                || activeEl?.tagName === "TEXTAREA"
                || (activeEl?.tagName === "SPAN" && activeEl.contentEditable === "true")
            ) {
                if (e.code === "Escape") {
                    (document.activeElement as any)?.blur();
                    containerRef.current?.focus?.();
                }

                // Defocus tags
                if (e.code === "KeyK" && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey) {
                    e.preventDefault();
                }
                return;
            }

            // Delete focused bookmark
            if (e.code === "KeyD" && !isModKey(e)) {
                e.preventDefault();
                const bmark = bmarks[focused];
                if (bmark) {
                    if (window.confirm(`Delete following bookmark?\n\n"${bmark.title}"\n`)) {
                        handleDelete(bmark.id);
                    }
                } else {
                    toast.error("bookmark not found");
                }
            }

            // Edit focused bookmark
            if (e.code === "KeyE" && !isModKey(e)) {
                e.preventDefault();
                const bmark = bmarks[focused];
                if (bmark) {
                    setEditingId(bmark.id);
                }
            }

            // Create new bookmark
            if (e.code === "KeyN" && !isModKey(e)) {
                e.preventDefault();
                setPastedUrl(undefined);
                setEditingId(-1);
                setCreating(true);
            }

            // Cancel edit
            if (e.code === "Escape" && !isModKey(e)) {
                e.preventDefault();
                if (editingId) {
                    setEditingId(-1);
                }

                if (creating) {
                    setCreating(false);
                    setPastedUrl(undefined);
                }
            }

            // Open focused bookmark
            if (e.code === "Enter" && !isModKey(e)) {
                e.preventDefault();
                (document.querySelector("#bmark-" + bmarks[focused]?.id + " .bmark-url") as any)?.click();
            }

            // Navigate right
            if (e.code === "KeyL" && !isModKey(e)) {
                e.preventDefault();
                setFocused(Math.min(bmarks.length - 1, focused + 1));
                if (editingId) setEditingId(-1);
            }

            // Navigate left
            if (e.code === "KeyH" && !isModKey(e)) {
                e.preventDefault();
                setFocused(Math.max(0, focused - 1));
                if (editingId) setEditingId(-1);
            }

            // Navigate up
            if (e.code === "KeyK" && !isModKey(e)) {
                e.preventDefault();

                if (focused - columns < 0) {
                    return;
                }

                if (editingId) setEditingId(-1);
                setFocused(Math.max(0, focused - columns));
            }

            // Focus tags
            if (e.code === "KeyK" && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey) {
                e.preventDefault();
                (document.querySelector(".header .tag-search") as any)?.focus();
            }

            // Navigate down
            if (e.code === "KeyJ" && !isModKey(e)) {
                e.preventDefault();
                const value = focused === -1 ? 0 : focused;

                if (value + columns > bmarks.length - 1) {
                    return;
                }

                if (editingId) setEditingId(-1);
                setFocused(Math.min(bmarks.length - 1, value + columns));
            }
        };

        const onPaste = (e: any) => {
            if (showSettings) {
                return;
            }

            // Check if we have image data in clipboard when editing
            if (editingId >= 0 && e.clipboardData.files && e.clipboardData.files.length > 0) {
                const file = e.clipboardData.files[0];
                if (file && file.type.startsWith('image/')) {
                    e.preventDefault();
                    e.stopPropagation();

                    toast.promise(
                        toBase64(file).then(b64 => {
                            const updateBmark: UpdateBmark = {
                                id: editingId,
                                image_b64: b64,
                            };
                            handleSave(updateBmark);
                        }),
                        {
                            loading: 'Uploading image...',
                            success: 'Image uploaded!',
                            error: 'Failed to upload image',
                        }
                    );
                    return;
                }
            }

            // Allow normal text paste into inputs when editing
            const currentActive = document.activeElement as HTMLInputElement;
            if (
                (currentActive?.tagName === "INPUT" && currentActive.type !== "checkbox")
                || currentActive?.tagName === "TEXTAREA"
                || (currentActive?.tagName === "SPAN" && currentActive.contentEditable === "true")
            ) {
                // Let the input handle the text paste normally
                return;
            }

            // Handle URL paste for creating new bookmarks (existing functionality)
            if (!creating && editingId < 0) {
                try {
                    const text = e.clipboardData.getData('text');
                    const url = new URL(text);

                    e.preventDefault();
                    setEditingId(-1);
                    setPastedUrl(url.toString());
                    setCreating(true);
                } catch (_) {
                    setPastedUrl(undefined);
                }
            }
        };

        document.addEventListener("keydown", onKeyDown);
        window.addEventListener("paste", onPaste);

        return () => {
            document.removeEventListener("keydown", onKeyDown);
            window.removeEventListener('paste', onPaste);
        };
    }, [focused, bmarks, editingId, columns, creating, showSettings, handleDelete, handleSave, setCreating, setEditingId, setPastedUrl, containerRef]);

    return {
        focused,
        setFocused,
    };
}