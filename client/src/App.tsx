import './App.css';
import { memo, MutableRefObject, useEffect, useRef, useState } from 'react';
import { Bmark, BookmarkCreate, Config, createBmark, deleteBmark, fetchBmarks, fetchConfig, fetchMeta, fetchTags, fetchTaskQueue, fetchTotal, updateBmark, UpdateBmark } from './api';
import { areEqual, VariableSizeGrid as Grid } from 'react-window';
import Header from './header';
import Bookmark, { CreateBookmark } from './bookmark';
import toast, { Toaster } from 'react-hot-toast';
import { findRunningTask, isModKey } from './helpers';
import { useDispatch } from 'react-redux';
import { update } from './store/taskQueueSlice';
import store from './store';

const MIN_ROW_HEIGHT = 450;

const DEFAULT_COLUMNS = 4;

function App() {
    const [bmarks, setBmarks] = useState<Bmark[]>([]);
    const [total, setTotal] = useState<number>(-1);
    const [tags, setTags] = useState<string[]>([]);
    const [config, setConfig] = useState<Config>();

    const dispatch = useDispatch();


    const [sizes, setSizes] = useState<number[]>([]);
    const [columns, setColumns] = useState(DEFAULT_COLUMNS);
    const [focused, setFocused] = useState(-1);

    const [renderKey, _setRenderKey] = useState(0);

    const rerenderList = () => {
        setSizes(new Array(Math.ceil(bmarks.length / columns)).fill(MIN_ROW_HEIGHT));
        _setRenderKey(Date.now())
    };

    const [loaded, setLoaded] = useState(false);
    const [editingId, setEditingId] = useState<number>();
    const [creating, setCreating] = useState(false);

    const [showAll, setShowAll] = useState(false);

    const gridRef = useRef<Grid<Bmark[]>>()

    const containerRef = useRef<HTMLDivElement>();
    const headerRef = useRef<HTMLDivElement>();

    // forms
    const [inputTags, _setInputTags] = useState("");
    const [inputTitle, _setInputTitle] = useState("");
    const [inputUrl, _setInputUrl] = useState("");
    const [inputDescription, _setInputDescription] = useState("");
    const formRefs = useRef({ inputTags: inputTags, inputTitle: inputTitle, inputUrl: inputUrl, inputDescription: inputDescription });

    const [pastedUrl, setPastedUrl] = useState<string>();

    const setInputTags = (val: string) => {
        formRefs.current.inputTags = val;
        return _setInputTags(val)
    }

    const setInputTitle = (val: string) => {
        formRefs.current.inputTitle = val;
        return _setInputTitle(val)
    }

    const setInputUrl = (val: string) => {
        formRefs.current.inputUrl = val;
        return _setInputUrl(val)
    }

    const setInputDescription = (val: string) => {
        formRefs.current.inputDescription = val;
        return _setInputDescription(val)
    }

    // misc
    const refreshTimerRef = useRef({ "1": 0, "2": 0, "3": 0, });

    function refreshTotal() {
        clearTimeout(refreshTimerRef.current["2"]);
        refreshTimerRef.current["2"] = setTimeout(() => {
            fetchTotal().then(setTotal);
        }, 1000) as any;
    }

    function refreshConfig() {
        return fetchConfig().then(setConfig)
    }

    function refreshTags() {
        return new Promise((resolve, reject) => {
            // HACK: tags are cached and revalidated asynchronously
            //       so we give backend some time to process them.
            //       300ms should be plenty
            setTimeout(() => {
                fetchTags().then(setTags).then(resolve).catch(reject);
            }, 300);
        });
    }

    function setCols() {
        const innerWidth = window.innerWidth;

        let cols = DEFAULT_COLUMNS;
        if (innerWidth < 768) {
            cols = 1
        } else if (innerWidth < 1024) {
            cols = 2
        } else if (innerWidth < 1280) {
            cols = 3
        }

        setColumns(cols);
    }

    async function refreshBmarks(opts: {
        scrollToTop?: boolean;
        notify?: boolean;
        resetSizes?: boolean;
        disableEditing?: boolean;
    } = {}) {
        const {
            scrollToTop = true,
            notify = true,
            resetSizes = true,
            disableEditing = false,
        } = opts;

        clearTimeout(refreshTimerRef.current["1"]);

        const shouldRefresh = formRefs.current.inputTags
            || formRefs.current.inputTitle
            || formRefs.current.inputUrl
            || formRefs.current.inputDescription
            || showAll;

        if (!shouldRefresh) {
            setBmarks([]);
            setEditingId(undefined);
            return
        }

        return new Promise((resolve, reject) => {
            refreshTimerRef.current["1"] = setTimeout(() => {
                const tLoading = notify ? toast.loading("loading") : "-1";

                return fetchBmarks({
                    tags: formRefs.current.inputTags.replaceAll(" ", ","),
                    title: inputTitle,
                    url: inputUrl,
                    description: inputDescription,
                    descending: true,
                }).then(bmarksResp => {
                    if (scrollToTop) {
                        gridRef.current?.scrollTo({ scrollTop: 0 });
                    }


                    if (disableEditing) {
                        setEditingId(undefined);
                    }
                    setCols()

                    if (resetSizes) {
                        setSizes(new Array(Math.ceil(bmarksResp.length / columns)).fill(MIN_ROW_HEIGHT));
                    }

                    setBmarks(curr => {
                        if (curr.length !== bmarksResp.length) {
                            if (bmarksResp.length > 0) {
                                setFocused(0);
                            } else {
                                setFocused(-1);
                            }
                        }

                        return bmarksResp
                    });
                    setLoaded(true);
                    resolve(bmarksResp);
                })
                    .catch(reject)
                    .finally(() => {
                        toast.dismiss(tLoading)
                    });

            }, 100) as any;
        })
    }

    useEffect(() => {
        refreshBmarks({ disableEditing: true });
        refreshTotal();


        const id = setInterval(() => {
            refreshConfig();
            refreshTags();

            refreshTaskQueue().then(changed => {
                if (changed) {
                    return refreshBmarks({ scrollToTop: false, notify: false, resetSizes: false, disableEditing: true })
                        .then(() => {
                            refreshTags();
                        })
                }
            });
        }, 1500);
        return () => {
            clearInterval(id)
        }
    }, [inputTags, inputTitle, inputUrl, inputDescription]);

    async function refreshTaskQueue(): Promise<boolean> {
        const tasks = await fetchTaskQueue();

        const taskQueue = store.getState().taskQueue.value
        dispatch(update(tasks));

        return taskQueue.now! != tasks.now

    }

    useEffect(() => {
        refreshConfig();
        refreshTags();
        refreshTaskQueue();
    }, []);

    useEffect(() => {
        fetchTotal().then(setTotal);

        const onResize = () => {
            clearTimeout(refreshTimerRef.current["3"]);
            refreshTimerRef.current["3"] = setTimeout(() => {
                setCols()
                rerenderList();
            }, 32) as any;
        };

        window.addEventListener("resize", onResize);

        return () => {
            window.removeEventListener("resize", onResize);
        }
    }, []);

    useEffect(() => {
        if (focused < 0) {
            return
        }

        const rowIndex = Math.floor(focused / columns);
        const columnIndex = Math.floor(focused % columns);

        gridRef.current?.scrollToItem({ columnIndex, rowIndex });
    }, [focused]);

    useEffect(() => {
        const onKeyDown = (e: KeyboardEvent) => {
            // defocus inputs
            if (document.activeElement?.tagName === "INPUT" || document.activeElement?.tagName === "TEXTAREA") {
                if (e.code === "Escape") {
                    (document.activeElement as any)?.blur()
                    containerRef.current?.focus?.();
                }

                // defocus tags
                if (e.code === "KeyK" && (e.ctrlKey || e.metaKey)) {
                    e.preventDefault();

                    if (document.activeElement?.closest(".tag-search")) {
                        (document.querySelector(".header .tag-search") as any)?.blur();
                    }
                }
                return
            }

            // edit focused bmark
            if (e.code === "KeyD" && !isModKey(e)) {
                e.preventDefault();
                const bmark = bmarks[focused];
                if (bmark) {
                    if (window.confirm(`Delete following bookmark?\n\n"${bmark.title}"\n`)) {
                        handleDelete(bmark.id)
                    }
                } else {
                    toast.error("bookmark not found");
                }

            }

            // edit focused bmark
            if (e.code === "KeyE" && !isModKey(e)) {
                e.preventDefault();
                const bmark = bmarks[focused];
                if (bmark) {
                    setEditingId(bmark.id)
                }
            }

            // create new
            if (e.code === "KeyN" && !isModKey(e)) {
                e.preventDefault();
                setPastedUrl(undefined);
                setEditingId(undefined);
                setCreating(true);
            }

            // cancel edit
            if (e.code === "Escape" && !isModKey(e)) {
                e.preventDefault();
                if (editingId) {
                    setEditingId(undefined);
                }

                if (creating) {
                    setCreating(false);
                    setPastedUrl(undefined);

                }
            }

            // open focus
            if (e.code === "Enter" && !isModKey(e)) {
                e.preventDefault();
                (document.querySelector("#bmark-" + bmarks[focused]?.id + " .bmark-url") as any)?.click()
            }

            // focus right
            if (e.code === "KeyL" && !isModKey(e)) {
                e.preventDefault();
                setFocused(Math.min(bmarks.length - 1, focused + 1))
                if (editingId) setEditingId(undefined);
            }

            // focus left
            if (e.code === "KeyH" && !isModKey(e)) {
                e.preventDefault();
                setFocused(Math.max(0, focused - 1))
                if (editingId) setEditingId(undefined);

            }

            // focus top
            if (e.code === "KeyK" && !isModKey(e)) {
                e.preventDefault();

                if (focused - columns < 0) {
                    return
                }

                if (editingId) setEditingId(undefined);


                setFocused(Math.max(0, focused - columns))
            }

            // focus tags
            if (e.code === "KeyK" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();

                (document.querySelector(".header .tag-search") as any)?.focus();
            }

            // focus bottom
            if (e.code === "KeyJ" && !isModKey(e)) {
                e.preventDefault();
                // skip row if going from -1
                const value = focused === -1 ? 0 : focused;

                if (value + columns > bmarks.length - 1) {
                    return
                }

                if (editingId) setEditingId(undefined);

                setFocused(Math.min(bmarks.length - 1, value + columns));
            }
        }

        const onPaste = (e: any) => {
            try {
                const text = e.clipboardData.getData('text');
                const url = new URL(text);

                const currentActive = document.activeElement?.tagName;
                if (currentActive === "INPUT" || currentActive === "TEXTAREA") {
                    return
                }
                if (creating || editingId) {
                    return
                }

                e.preventDefault();
                setPastedUrl(url.toString());
                setEditingId(undefined);
                setCreating(true);
            } catch (_) {
                setPastedUrl(undefined);
                // pass
            }
        };


        document.addEventListener("keydown", onKeyDown);
        window.addEventListener("paste", onPaste);

        return () => {
            document.removeEventListener("keydown", onKeyDown);
            window.removeEventListener('paste', onPaste);
        }
    }, [focused, bmarks, editingId, columns, creating]);

    const handleDelete = (id: number) => {
        const currTask = findRunningTask(id)
        if (currTask) {
            toast.error("cannot delete while being processed");
            return
        }

        toast.promise(deleteBmark(id).then(() => {
            refreshTags();
            refreshTotal();
            return refreshBmarks({ scrollToTop: false, notify: false });
        }), {
            loading: 'Deleting...',
            success: 'Deleted!',
            error: (err) => err.message,
        });
    }

    function handleFetchMeta(id: number) {
        const currTask = findRunningTask(id)
        if (currTask) {
            toast.error("cannot update while being processed.");
            return
        }

        toast.promise(
            fetchMeta(id).then(() => {
                refreshTaskQueue();
                setEditingId(undefined);
                return refreshBmarks({ scrollToTop: false, notify: false, resetSizes: false, disableEditing: false })
                    .then(() => {
                        refreshTags();
                    })
            }),
            {
                loading: 'Requesting metadata refetch...',
                success: 'Requested metadata refetch!',
                error: (err) => err.message,
            }
        );
    }

    function handleSave(update: UpdateBmark) {
        const currTask = findRunningTask(update.id);
        if (currTask) {
            toast.error("cannot update while being processed.");
            return
        }

        toast.promise(
            updateBmark(update.id, update).then(() => {
                return refreshBmarks({ scrollToTop: false, notify: false, resetSizes: false, disableEditing: true })
                    .then(() => {
                        refreshTags();
                    })
            }),
            {
                loading: 'Saving...',
                success: 'Saved!',
                error: (err) => err.message,
            }
        );
    }

    const onCreating = () => {
        setPastedUrl(undefined);
        setEditingId(undefined);
        setCreating(true);
    };

    const onCreate = (bmark: BookmarkCreate) => {
        toast.promise(
            createBmark({
                ...bmark,
            }).then(() => {
                refreshTaskQueue();
                setTimeout(() => {
                    refreshTaskQueue();
                }, 100);

                setCreating(false);
                return refreshBmarks({ scrollToTop: true, notify: false })
                    .then(() => {
                        refreshTags();
                    })
            }),
            {
                loading: 'Creating bookmark...',
                success: 'Created!',
                error: (err) => err.message,
            }
        );

    };

    if (!config) {
        return <div>loading...</div>
    }

    return (
        <>
            {creating && <div
                onClick={e => {
                    e.preventDefault();
                    setCreating(false);
                }}
                className="fixed z-50 cursor-pointer motion-safe:backdrop-blur-xl bg-gray-900/40 top-0 left-0 right-0 bottom-0 flex"
            >
                <div onClick={e => e.stopPropagation()} className="m-auto z-50 h-auto w-full max-h-screen max-w-screen-lg">
                    <CreateBookmark config={config} tagList={tags} className="shadow-[0_0px_50px_0px_rgba(0,0,0,0.3)]" defaultUrl={pastedUrl} onCreate={onCreate} />
                </div>
            </div>}
            <div className="dark:bg-gray-900 dark:text-gray-100 h-dvh overflow-hidden">
                <Header
                    config={config}
                    tagList={tags}
                    tags={inputTags}
                    onRef={ref => headerRef.current = ref ?? undefined}
                    onTags={setInputTags}
                    title={inputTitle}
                    onNewBookmark={onCreating}
                    onTitle={setInputTitle}
                    url={inputUrl}
                    onUrl={setInputUrl}
                    description={inputDescription}
                    onDescription={setInputDescription}
                    total={total}
                    count={loaded ? bmarks.length : -1}
                    columns={columns}
                    onColumns={(v) => {
                        setColumns(v);
                        rerenderList();
                    }}
                />
                <div
                    key={renderKey}
                    className="dark:bg-gray-900 overflow-y-scroll overflow-x-hidden h-screen xs:m-0 md:mx-5 h-full"
                    ref={ref => containerRef.current = ref ?? undefined}
                >
                    {(containerRef.current && headerRef.current && <GridView
                        tagList={tags}
                        gridRef={gridRef}
                        containerRef={containerRef.current}
                        columns={columns}
                        sizes={sizes}
                        bmarks={bmarks}
                        setSizes={setSizes}
                        headerRef={headerRef.current}
                        focused={focused}
                        setEditingId={setEditingId}
                        editingId={editingId}
                        handleDelete={handleDelete}
                        handleSave={handleSave}
                        handleFetchMeta={handleFetchMeta}
                        config={config}
                    />)}
                </div>
                <div className="p-5">
                </div>
            </div>
            <Toaster toastOptions={{ style: { background: '#505560', color: '#E5E7EB', } }} position="bottom-right" />
        </>
    );
}

interface GridViewProps {
    gridRef: MutableRefObject<Grid<Bmark[]> | undefined>;
    columns: number;
    sizes: number[];
    bmarks: Bmark[];
    setSizes: (v: number[]) => void;
    focused: number;
    setEditingId: (v?: number) => void;
    editingId?: number;
    handleDelete: (id: number) => void;
    handleSave: (update: UpdateBmark) => void;
    handleFetchMeta: (id: number) => void;
    config: Config
    tagList: string[];
    containerRef: HTMLDivElement;
    headerRef: HTMLDivElement;
}

function GridView(props: GridViewProps) {
    const containerRect = props.containerRef.getBoundingClientRect();
    const headerRect = props.headerRef.getBoundingClientRect();

    const {
        gridRef,
        columns,
        sizes,
        bmarks,
        setSizes,
        focused,
        setEditingId,
        editingId,
        handleDelete,
        handleSave,
        handleFetchMeta,
    } = props;

    const innerHeight = window.innerHeight;
    return <Grid
        ref={ref => gridRef.current = (ref as any) ?? undefined}
        columnCount={columns}
        columnWidth={_ => (containerRect.width) / columns}
        rowHeight={rowIndex => {
            return Math.max(sizes[rowIndex], MIN_ROW_HEIGHT) || MIN_ROW_HEIGHT
        }}
        rowCount={Math.ceil(bmarks.length / columns)}
        height={innerHeight}
        itemData={{
            bmarks, columns, sizes,
            setSizes, gridRef, headerRect,
            config: props.config,
            tagList: props.tagList,
            focused, editingId, setEditingId,
            handleDelete, handleSave, handleFetchMeta
        }}
        // itemKey={data => {
        //     const idx = (data.rowIndex * columns) + data.columnIndex;
        //     return data.data.bmarks[idx]?.id
        // }}
        width={containerRect.width}
        className="dark:bg-gray-900"
    >
        {Row}
    </Grid>

}

const Row = memo(({ columnIndex, rowIndex, style, data }: any) => {
    const {
        bmarks,
        columns,
        sizes,
        setSizes,
        gridRef,
        headerRect,
        config,
        tagList,
        focused,
        editingId,
        setEditingId,
        handleDelete,
        handleSave,
        handleFetchMeta
    } = data;

    const idx = (rowIndex * columns) + columnIndex;
    const bmark: Bmark = bmarks[idx]
    if (!bmark) return null
    const onSize = (_: number, h: number) => {
        const current = sizes[rowIndex];
        h = Math.max(MIN_ROW_HEIGHT, h + 20);
        if (current && current >= h) {
            return
        }

        const sizesNew = [...sizes];
        sizesNew[rowIndex] = h;

        setSizes(sizesNew);
        gridRef.current?.resetAfterIndices({ columnIndex, rowIndex });
    };

    style.marginTop = headerRect?.height + "px"

    return <Bookmark
        config={config}
        tagList={tagList}
        focused={focused === idx}
        onSize={onSize}
        style={style}
        isEditing={bmark.id === editingId}
        setEditing={(val) => {
            if (!val) {
                setEditingId(undefined);
            } else {
                setEditingId(bmark.id);
            }
        }}
        bmark={bmark}
        onDelete={() => handleDelete(bmark.id)}
        onSave={handleSave}
        onFetchMeta={() => handleFetchMeta(bmark.id)}
    />
}, areEqual)

export default App;
