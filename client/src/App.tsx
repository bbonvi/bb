import './App.css';
import { memo, MutableRefObject, useEffect, useRef, useState } from 'react';
import { Bmark, Config, UpdateBmark } from './api';
import * as api from './api';
import { areEqual, VariableSizeGrid as Grid } from 'react-window';
import Header from './header';
import Bookmark, { CreateBookmark } from './bookmark';
import { Toaster } from 'react-hot-toast';
import { deepClone, useLocalStorage } from './helpers';
import { useDispatch, useSelector } from 'react-redux';
import * as taskQueueSlice from './store/taskQueueSlice';
import * as globalSlice from './store/globalSlice';
import store, { RootState } from './store';
import Settings, { SettingsState } from './settings';
import { defaultWorkspace } from './workspaces';
import IframePopup from './popup';
import { useBookmarkSearch } from './hooks/useBookmarkSearch';
import { useKeyboardNavigation } from './hooks/useKeyboardNavigation';
import { useBookmarkCRUD } from './hooks/useBookmarkCRUD';
import { useGridLayout } from './hooks/useGridLayout';
import LoginGate, { getAuthToken } from './LoginGate';

type AuthState = 'checking' | 'required' | 'authenticated';

function useAuth() {
    const [authState, setAuthState] = useState<AuthState>('checking');

    const checkAuth = async () => {
        const hasToken = !!getAuthToken();

        try {
            await api.fetchConfig();
            setAuthState('authenticated');
        } catch (err: any) {
            if (err?.message?.includes('401')) {
                setAuthState(hasToken ? 'checking' : 'required');
                if (hasToken) {
                    // Token exists but might be invalid - axios interceptor will handle reload
                }
            } else {
                // Network error or other issue - assume authenticated (will fail gracefully)
                setAuthState('authenticated');
            }
        }
    };

    useEffect(() => {
        checkAuth();
    }, []);

    const handleLogin = () => {
        setAuthState('checking');
        checkAuth();
    };

    return { authState, handleLogin };
}

function AuthWrapper() {
    const { authState, handleLogin } = useAuth();

    if (authState === 'checking') {
        return <div className="dark:bg-gray-900 dark:text-gray-100 h-screen flex items-center justify-center">loading...</div>;
    }
    if (authState === 'required') {
        return <LoginGate onLogin={handleLogin} />;
    }

    return <App />;
}

function App() {
    const [total, setTotal] = useState<number>(-1);
    const [tags, setTags] = useState<string[]>([]);
    const [config, setConfig] = useState<Config>();

    const [settings, _saveSettings] = useLocalStorage<SettingsState>("settings", {
        workspaceState: { workspaces: [defaultWorkspace()], currentWorkspace: 0 },
    });

    function saveSettings(settings: SettingsState) {
        _saveSettings(settings);
        setSettingsUpdated(Date.now());
    }

    const [shuffleSeed, setShuffleSeed] = useState(Date.now());
    const [showAll, setShowAll] = useState(false);
    const [showIframePopup, setShowIframePopup] = useState<Bmark | undefined | null>();
    const [shuffle, setShuffle] = useState(false);
    const [settingsUpdated, setSettingsUpdated] = useState(Date.now());
    const [creating, setCreating] = useState(false);
    const [showSettings, setShowSettings] = useState(false);
    const [pastedUrl, setPastedUrl] = useState<string>();

    const dispatch = useDispatch();
    const editingId = useSelector((state: RootState) => state.global.editing);
    const setEditingId = (idx: number) => dispatch(globalSlice.setEditing(idx));

    useEffect(() => {
        setShuffleSeed(Date.now());
    }, [shuffle]);

    const gridRef = useRef<Grid<Bmark[]>>()
    const containerRef = useRef<HTMLDivElement>();
    const headerRef = useRef<HTMLDivElement>();

    // Custom hooks
    const searchHook = useBookmarkSearch({ settings, settingsUpdated, showAll });
    const gridHook = useGridLayout({ bmarks: searchHook.bmarksFiltered, shuffleSeed, shuffle });

    const refreshTags = (): Promise<void> => {
        return new Promise((resolve, reject) => {
            setTimeout(() => {
                api.fetchTags().then(t => {
                    if (JSON.stringify(t) !== JSON.stringify(tags)) {
                        setTags(t)
                    }
                }).then(() => resolve()).catch(reject);
            }, 300);
        });
    };

    const crudHook = useBookmarkCRUD({
        setEditingId,
        setCreating,
        refreshTags,
        refreshTotal,
    });

    const keyboardHook = useKeyboardNavigation({
        bmarks: gridHook.bmarksShuffled,
        columns: gridHook.columns,
        showSettings,
        creating,
        editingId,
        setCreating,
        setEditingId,
        setPastedUrl,
        handleDelete: crudHook.handleDelete,
        handleSave: crudHook.handleSave,
        containerRef: containerRef as React.RefObject<HTMLDivElement>,
    });

    const refreshTimerRef = useRef({ "1": 0, "2": 0 });

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


    const refreshAll = async () => {
        return Promise.all([
            // refresh bookmarks
            searchHook.getBmarks({
                tags: searchHook.inputTags,
                title: searchHook.inputTitle,
                description: searchHook.inputDescription,
                url: searchHook.inputUrl,
                keyword: searchHook.inputKeyword.replace(/^#/, "")
            })
                .then(searchHook.updateBmarksIfNeeded),

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
        refreshAll();
    }, []);

    // Set initial focus when bookmarks are first loaded
    useEffect(() => {
        if (keyboardHook.focused === -1 && gridHook.bmarksShuffled.length > 0) {
            keyboardHook.setFocused(0);
        }
    }, [gridHook.bmarksShuffled.length, keyboardHook.focused]);

    useEffect(() => {
        searchHook.refreshBmarks().then(changed => {
            if (changed) {
                setEditingId(-1);
                gridRef.current?.scrollTo({ scrollTop: 0 });
                keyboardHook.setFocused(0);
            }
        });

        const timerId = setInterval(() => {
            // do not refresh data as long as something's being saved/delete/refreshed
            if (crudHook.updating.current > 0 || store.getState().global.editing >= 0) {
                return
            }

            refreshAll();
        }, 3000);

        return () => {
            clearInterval(timerId)
        }
    }, [searchHook.inputTags, searchHook.inputTitle, searchHook.inputUrl, searchHook.inputDescription, searchHook.inputKeyword, showAll, settingsUpdated]);


    useEffect(() => {
        refreshConfig();
        refreshTags();
        crudHook.refreshTaskQueue();
    }, []);

    useEffect(() => {
        api.fetchTotal().then(setTotal);
    }, []);

    useEffect(() => {
        if (keyboardHook.focused < 0) {
            return
        }

        const rowIndex = Math.floor(keyboardHook.focused / gridHook.columns);
        const columnIndex = Math.floor(keyboardHook.focused % gridHook.columns);

        gridRef.current?.scrollToItem({ columnIndex, rowIndex });
    }, [keyboardHook.focused, gridHook.columns]);


    const onCreating = () => {
        setPastedUrl(undefined);
        setEditingId(-1);
        setCreating(true);
    };

    if (!config) {
        return <div>loading...</div>
    }

    return (
        <>
            {showSettings && !creating && <div
                onClick={e => {
                    e.preventDefault();
                    setShowSettings(false);
                }}
                className="fixed z-50 cursor-pointer motion-safe:backdrop-blur-xl bg-gray-900/40 top-0 left-0 right-0 bottom-0"
            >
                <div onClick={e => e.stopPropagation()} className="m-auto z-50 w-full max-w-screen-lg cursor-auto py-2 h-full">
                    <Settings settings={settings} onSave={(settings) => saveSettings(settings)} tags={tags} />
                </div>
            </div>}

            {/* Iframe Popup */}
            {showIframePopup &&
                <IframePopup
                    url={showIframePopup.url}
                    isOpen={!!showIframePopup}
                    onClose={() => setShowIframePopup(null)}
                />}
            {creating && <div
                onClick={e => {
                    e.preventDefault();
                    setCreating(false);
                }}
                className="fixed z-50 cursor-pointer motion-safe:backdrop-blur-xl bg-gray-900/40 top-0 left-0 right-0 bottom-0 flex"
            >
                <div onClick={e => e.stopPropagation()} className="m-auto z-50 h-auto w-full max-h-screen max-w-screen-lg">
                    <CreateBookmark
                        handleKeyDown={!showSettings}
                        config={config}
                        tagList={tags}
                        className="shadow-[0_0px_50px_0px_rgba(0,0,0,0.3)]"
                        defaultUrl={pastedUrl}
                        onCreate={crudHook.onCreate}
                    />
                </div>
            </div>}
            <div className="dark:bg-gray-900 dark:text-gray-100 h-screen p-0 [scrollbar-gutter:stable]">
                <Header
                    settings={deepClone(settings)}
                    onSaveSettings={saveSettings}
                    openSettings={() => setShowSettings(true)}
                    config={config}
                    shuffle={shuffle}
                    setShuffle={setShuffle}
                    tagList={tags}
                    tags={searchHook.inputTags}
                    onRef={ref => headerRef.current = ref ?? undefined}
                    onTags={searchHook.setInputTags}
                    onShowAll={setShowAll}
                    showAll={showAll}
                    title={searchHook.inputTitle}
                    onNewBookmark={onCreating}
                    onTitle={searchHook.setInputTitle}
                    url={searchHook.inputUrl}
                    onUrl={searchHook.setInputUrl}
                    description={searchHook.inputDescription}
                    onDescription={searchHook.setInputDescription}
                    keyword={searchHook.inputKeyword}
                    onKeyword={searchHook.setInputKeyword}
                    total={total}
                    count={gridHook.bmarksShuffled.length}
                    columns={gridHook.columns}
                    onColumns={(v) => {
                        gridHook.setColumns(v);
                        gridHook.rerenderList();
                    }}
                />
                <div
                    className="dark:bg-gray-900 overflow-y-hidden overflow-x-hidden mx-0 px-0"
                    ref={ref => containerRef.current = ref ?? undefined}
                >
                    {(containerRef.current && headerRef.current && <GridView
                        tagList={tags}
                        setShowIframePopup={setShowIframePopup}
                        key={gridHook.renderKey}
                        gridRef={gridRef}
                        containerRef={containerRef.current}
                        columns={gridHook.columns}
                        bmarks={gridHook.bmarksShuffled}
                        headerRef={headerRef.current}
                        focused={keyboardHook.focused}
                        setEditingId={setEditingId}
                        editingId={editingId}
                        handleDelete={crudHook.handleDelete}
                        handleSave={crudHook.handleSave}
                        handleFetchMeta={crudHook.handleFetchMeta}
                        config={config}
                        minRowHeight={gridHook.MIN_ROW_HEIGHT}
                    />)}
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
    setShowIframePopup: (bmark: Bmark) => void;
    minRowHeight: number;
}

function GridView(props: GridViewProps) {
    const [containerRect, setContainerRect] = useState(props.containerRef.getBoundingClientRect());
    const headerRect = props.headerRef.getBoundingClientRect();
    const sizes = useSelector((state: RootState) => state.global.sizes);

    useEffect(() => {
        setContainerRect(props.containerRef.getBoundingClientRect());
    }, [props]);

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
        setShowIframePopup,
    } = props;

    const innerHeight = window.innerHeight;
    return <Grid
        ref={ref => gridRef.current = (ref as any) ?? undefined}
        columnCount={props.columns}
        columnWidth={_ => Math.ceil((containerRect.width - 30) / props.columns)}
        rowHeight={rowIndex => {
            const idx = (rowIndex * props.columns) + 0;
            return Math.max(...props.bmarks.slice(idx, idx + props.columns).map(b => sizes[b.id.toString()] ?? props.minRowHeight));
        }}
        rowCount={Math.ceil(props.bmarks.length / props.columns)}
        height={innerHeight}
        itemData={{
            bmarks: props.bmarks, columns: props.columns,
            gridRef, headerRect,
            config: props.config,
            tagList: props.tagList,
            focused: props.focused, editingId: props.editingId, setEditingId: props.setEditingId,
            handleDelete: props.handleDelete, handleSave: props.handleSave, handleFetchMeta: props.handleFetchMeta,
            setShowIframePopup,
            minRowHeight: props.minRowHeight,
        }}
        itemKey={data => {
            const idx = (data.rowIndex * props.columns) + data.columnIndex;
            return data.data.bmarks[idx]?.id ?? (data.rowIndex.toString + "-" + data.columnIndex.toString())
        }}
        width={Math.ceil(containerRect.width)}
        className="dark:bg-gray-900 [scrollbar-gutter:stable]"
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
        handleFetchMeta,
        setShowIframePopup,
    } = data;
    const sizes = store.getState().global.sizes;

    const idx = (rowIndex * columns) + columnIndex;
    const bmark: Bmark = bmarks[idx]
    if (!bmark) return null
    const onSize = (_: number, h: number) => {
        const current = sizes[bmark.id.toString()];
        h = Math.max(data.minRowHeight || 400, h + 20);
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
        setShowIframePopup={setShowIframePopup}
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

export default AuthWrapper;
