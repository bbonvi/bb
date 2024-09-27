import './App.css';
import { memo, MutableRefObject, useEffect, useMemo, useRef, useState } from 'react';
import { Bmark, BookmarkCreate, Config, UpdateBmark } from './api';
import * as api from './api';
import { areEqual, VariableSizeGrid as Grid } from 'react-window';
import Header from './header';
import Bookmark, { CreateBookmark } from './bookmark';
import toast, { Toaster } from 'react-hot-toast';
import { findRunningTask, isModKey } from './helpers';
import { useDispatch, useSelector } from 'react-redux';
import * as taskQueueSlice from './store/taskQueueSlice';
import * as bmarksSlice from './store/bmarksSlice';
import * as globalSlice from './store/globalSlice';
import store, { RootState } from './store';
import isEqual from 'lodash.isequal';

const MIN_ROW_HEIGHT = 450;

const DEFAULT_COLUMNS = 4;

function App() {
    const [total, setTotal] = useState<number>(-1);
    const [tags, setTags] = useState<string[]>([]);
    const [config, setConfig] = useState<Config>();

    const updating = useRef(0);

    const dispatch = useDispatch();
    const bmarks = useSelector((state: RootState) => state.bmarks.value);
    const focused = useSelector((state: RootState) => state.global.focusedIdx);
    const editingId = useSelector((state: RootState) => state.global.editing);

    const setBmarks = (bmarks: Bmark[]) => dispatch(bmarksSlice.updateAll(bmarks));
    const setFocused = (idx: number) => dispatch(globalSlice.setFocusedIdx(idx))
    const setEditingId = (idx: number) => dispatch(globalSlice.setEditing(idx))

    const [ignoreHidden, setIgnoreHidden] = useState(false);

    const [sizes, setSizes] = useState<{ [keyof: string]: number }>({});
    const [columns, setColumns] = useState(DEFAULT_COLUMNS);
    // const [focused, setFocused] = useState(-1);

    const [renderKey, _setRenderKey] = useState(0);

    const rerenderList = () => {
        // setSizes(new Array(Math.ceil(bmarks.length / columns)).fill(MIN_ROW_HEIGHT));
        _setRenderKey(Date.now())
    };

    const hiddenByDefault: string[] = useMemo(() => {
        if (ignoreHidden) {
            return []
        }

        return config?.hidden_by_default ?? [];

    }, [ignoreHidden, config]);


    const [loaded, setLoaded] = useState(false);
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
            api.fetchTotal().then(t => {
                if (t !== total) {
                    setTotal(t)
                }
            });
        }, 1000) as any;
    }

    function refreshConfig() {
        return api.fetchConfig().then((conf) => {
            if (JSON.stringify(conf) !== JSON.stringify(config)) {
                return setConfig(conf)
            }
        })
    }

    function refreshTags() {
        return new Promise((resolve, reject) => {
            // HACK: tags are cached and revalidated asynchronously
            //       so we give backend some time to process them.
            //       300ms should be plenty
            setTimeout(() => {
                api.fetchTags().then(t => {
                    if (JSON.stringify(t) !== JSON.stringify(tags)) {
                        setTags(t)
                    }
                }).then(resolve).catch(reject);
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
        return cols;
    }

    async function hasBmarksChanged(bmarksList: Bmark[]) {
    }

    // async function handleBmarks(
    //     bmarksList: Bmark[],
    // )

    const getBmarks = async (props: {
        ignoreHiddenTags: boolean,
        tags: string,
        title: string,
        url: string,
        description: string,
    }) => {
        const tagsFetch = props.tags.trim().replaceAll(" ", ",").split(",");

        const shouldRefresh = props.tags.length
            || props.title
            || props.url
            || props.description
            || showAll;

        if (!shouldRefresh) {
            return []
        }


        if (!props.ignoreHiddenTags) {
            hiddenByDefault.forEach(ht => {
                if (!tagsFetch.find(t => ht === t || t.includes(ht + "/"))) {
                    tagsFetch.push("-" + ht)
                }
            });
        }

        return api.fetchBmarks({
            tags: tagsFetch.join(","),
            title: props.title,
            url: props.url,
            description: props.description,
            descending: true,
        })
    }

    async function _refreshBmarks(opts: {
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
            // setEditingId(undefined);
            return
        }

        return new Promise((resolve, reject) => {
            refreshTimerRef.current["1"] = setTimeout(() => {
                const tLoading = notify ? toast.loading("loading") : "-1";

                const tagsFetch = formRefs.current.inputTags.trim().replaceAll(" ", ",").split(",");

                if (!ignoreHidden) {
                    hiddenByDefault.forEach(ht => {
                        if (!tagsFetch.find(t => ht === t || t.includes(ht + "/"))) {
                            tagsFetch.push("-" + ht)
                        }
                    });
                }

                return api.fetchBmarks({
                    tags: tagsFetch.join(","),
                    title: inputTitle,
                    url: inputUrl,
                    description: inputDescription,
                    descending: true,
                }).then(bmarksResp => {

                    if (scrollToTop) {
                        gridRef.current?.scrollTo({ scrollTop: 0 });
                    }


                    if (disableEditing) {
                        setEditingId(-1);
                    }

                    if (resetSizes) {
                        // setSizes(new Array(Math.ceil(bmarksResp.length / columns)).fill(MIN_ROW_HEIGHT));
                    }

                    const currBmarks = store.getState().bmarks.value;
                    if (!isEqual(currBmarks, bmarksResp)) {
                        setCols()
                        if (currBmarks.length !== bmarksResp.length) {
                            if (bmarksResp.length > 0) {
                                setFocused(0);
                            } else {
                                setFocused(-1);
                            }
                        }

                        setBmarks(bmarksResp);
                    }

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
        // refreshBmarks({ disableEditing: true, notify: false, resetSizes: false, scrollToTop: true });
    }, [hiddenByDefault]);

    const refreshBmarks = () => getBmarks({
        ignoreHiddenTags: ignoreHidden,
        tags: inputTags,
        title: inputTitle,
        description: inputDescription,
        url: inputUrl,
    })
        .then(bmarks => {
            if (!isEqual(store.getState().bmarks.value, bmarks)) {
                dispatch(bmarksSlice.updateAll(bmarks))
                return true
            }

            return false
        });

    const refreshAll = async () => {
        return Promise.all([
            // refresh bookmarks
            getBmarks({
                ignoreHiddenTags: ignoreHidden,
                tags: inputTags,
                title: inputTitle,
                description: inputDescription,
                url: inputUrl,
            })
                .then(bmarks => !isEqual(store.getState().bmarks.value, bmarks) ? dispatch(bmarksSlice.updateAll(bmarks)) : null),

            // refresh config
            api.fetchConfig()
                .then((conf) => JSON.stringify(conf) !== JSON.stringify(config) ? setConfig(conf) : null),

            // refresh tags
            api.fetchTags()
                .then(t => JSON.stringify(t) !== JSON.stringify(tags) ? setTags(t) : null),

            // refresh task queue
            api.fetchTaskQueue()
                .then(tq => store.getState().taskQueue.value.now !== tq.now ? dispatch(taskQueueSlice.update(tq)) : null),

            // refresh total count
            api.fetchTotal()
                .then(t => t !== total ? setTotal(t) : null),
        ]);
    }

    useEffect(() => {
        refreshAll().then(() => {
            setCols()
            const len = store.getState().bmarks.value.length;
            // setSizes(new Array(Math.ceil(len / columns)).fill(MIN_ROW_HEIGHT));
        });
    }, []);

    useEffect(() => {
        refreshBmarks().then(changed => {
            if (changed) {
                setEditingId(-1);
                gridRef.current?.scrollTo({ scrollTop: 0 });
                setFocused(0);
            }
        });

        const timerId = setInterval(() => {
            // do not refresh data as long as something's being saved/delete/refreshed
            if (updating.current > 0 || store.getState().global.editing >= 0) {
                return
            }

            refreshAll();
        }, 3000);

        return () => {
            clearInterval(timerId)
        }
    }, [inputTags, inputTitle, inputUrl, inputDescription, ignoreHidden]);

    async function refreshTaskQueue(): Promise<boolean> {
        const tasks = await api.fetchTaskQueue();

        const taskQueue = store.getState().taskQueue.value
        dispatch(taskQueueSlice.update(tasks));

        return taskQueue.now! != tasks.now

    }

    useEffect(() => {
        refreshConfig();
        refreshTags();
        refreshTaskQueue();
    }, []);

    useEffect(() => {
        api.fetchTotal().then(setTotal);

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
            if (
                document.activeElement?.tagName === "INPUT"
                || document.activeElement?.tagName === "TEXTAREA"
                || (document.activeElement?.tagName === "SPAN" && (document.activeElement as any).contentEditable === "true")
            ) {
                if (e.code === "Escape") {
                    (document.activeElement as any)?.blur()
                    containerRef.current?.focus?.();
                }

                // defocus tags
                if (e.code === "KeyK" && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey) {
                    e.preventDefault();

                    // if (document.activeElement?.closest(".tag-search")) {
                    //     (document.querySelector(".header .tag-search") as any)?.blur();
                    // }
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
                setEditingId(-1);
                setCreating(true);
            }

            // cancel edit
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

            // open focus
            if (e.code === "Enter" && !isModKey(e)) {
                e.preventDefault();
                (document.querySelector("#bmark-" + bmarks[focused]?.id + " .bmark-url") as any)?.click()
            }

            // focus right
            if (e.code === "KeyL" && !isModKey(e)) {
                e.preventDefault();
                setFocused(Math.min(bmarks.length - 1, focused + 1))
                if (editingId) setEditingId(-1);
            }

            // focus left
            if (e.code === "KeyH" && !isModKey(e)) {
                e.preventDefault();
                setFocused(Math.max(0, focused - 1))
                if (editingId) setEditingId(-1);

            }

            // focus top
            if (e.code === "KeyK" && !isModKey(e)) {
                e.preventDefault();

                if (focused - columns < 0) {
                    return
                }

                if (editingId) setEditingId(-1);


                setFocused(Math.max(0, focused - columns))
            }

            // focus tags
            if (e.code === "KeyK" && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey) {
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

                if (editingId) setEditingId(-1);

                setFocused(Math.min(bmarks.length - 1, value + columns));
            }
        }

        const onPaste = (e: any) => {
            try {
                const text = e.clipboardData.getData('text');
                const url = new URL(text);

                const currentActive = document.activeElement?.tagName;
                if (
                    currentActive === "INPUT"
                    || currentActive === "TEXTAREA"
                    || (document.activeElement?.tagName === "SPAN" && (document.activeElement as any).contentEditable === "true")
                ) {
                    return
                }
                if (creating || editingId) {
                    return
                }

                e.preventDefault();
                setPastedUrl(url.toString());
                setEditingId(-1);
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

        updating.current += 1;
        toast.promise(api.deleteBmark(id).then(() => {
            dispatch(bmarksSlice.remove({ id }));

            setEditingId(-1);
            refreshTags();
            refreshTotal();
        }).finally(() => {
            updating.current -= 1;
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
            api.fetchMeta(id).then(() => {
                setEditingId(-1);
                refreshTaskQueue()
                    .then(() => setTimeout(refreshTaskQueue, 200));
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

        updating.current += 1;

        toast.promise(
            api.updateBmark(update.id, update).then((bmark) => {
                dispatch(bmarksSlice.update(bmark));
                setEditingId(-1);
                refreshTags();
            }).finally(() => {
                updating.current -= 1;
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
        setEditingId(-1);
        setCreating(true);
    };

    const onCreate = (bmark: BookmarkCreate) => {
        updating.current += 1;
        toast.promise(
            api.createBmark({
                ...bmark,
            }).then((bmark) => {
                dispatch(bmarksSlice.create(bmark));
                setCreating(false);
                setTimeout(() => {
                    refreshTaskQueue();
                }, 100);
                refreshTags();
            }).finally(() => {
                updating.current -= 1;
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
                    <CreateBookmark hiddenByDefault={hiddenByDefault} config={config} tagList={tags} className="shadow-[0_0px_50px_0px_rgba(0,0,0,0.3)]" defaultUrl={pastedUrl} onCreate={onCreate} />
                </div>
            </div>}
            <div className="dark:bg-gray-900 dark:text-gray-100 h-dvh overflow-hidden">
                <Header
                    config={config}
                    tagList={tags}
                    tags={inputTags}
                    onRef={ref => headerRef.current = ref ?? undefined}
                    onTags={setInputTags}
                    hiddenByDefault={hiddenByDefault}
                    setIgnoreHidden={setIgnoreHidden}
                    ignoreHidden={ignoreHidden}
                    title={inputTitle}
                    onNewBookmark={onCreating}
                    onTitle={setInputTitle}
                    url={inputUrl}
                    onUrl={setInputUrl}
                    description={inputDescription}
                    onDescription={setInputDescription}
                    total={total}
                    count={bmarks.length}
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
                        bmarks={bmarks}
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
    bmarks: Bmark[];
    focused: number;
    setEditingId: (v: number) => void;
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
    const sizes = useSelector((state: RootState) => state.global.sizes);

    const {
        gridRef,
        columns,
        bmarks,
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
            const idx = (rowIndex * columns) + 0;
            return Math.max(...bmarks.slice(idx, idx + columns).map(b => sizes[b.id.toString()] ?? MIN_ROW_HEIGHT));
        }}
        rowCount={Math.ceil(bmarks.length / columns)}
        height={innerHeight}
        itemData={{
            bmarks, columns,
            gridRef, headerRect,
            config: props.config,
            tagList: props.tagList,
            focused, editingId, setEditingId,
            handleDelete, handleSave, handleFetchMeta
        }}
        itemKey={data => {
            const idx = (data.rowIndex * columns) + data.columnIndex;
            return data.data.bmarks[idx]?.id ?? (data.rowIndex.toString + "-" + data.columnIndex.toString())
        }}
        width={containerRect.width}
        className="dark:bg-gray-900"
    >
        {Row}
    </Grid>

}

const Row = memo(({ columnIndex, rowIndex, style, data }: any) => {
    const dispatch = useDispatch();

    const {
        bmarks,
        columns,
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
    const sizes = store.getState().global.sizes;

    const idx = (rowIndex * columns) + columnIndex;
    const bmark: Bmark = bmarks[idx]
    if (!bmark) return null
    const onSize = (_: number, h: number) => {
        const current = sizes[bmark.id.toString()];
        h = Math.max(MIN_ROW_HEIGHT, h + 20);
        if (current && current >= h) {
            return
        }

        dispatch(globalSlice.setSize({ id: bmark.id, height: h }))

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
                setEditingId(-1);
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
