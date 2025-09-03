import { DragEventHandler, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { toast } from "react-hot-toast";
import { useSelector } from "react-redux";
import { Bmark, BookmarkCreate, Config, Task, UpdateBmark } from "./api";
import Button, { ButtonConfirm } from "./button";
import { TagInput } from "./components";
import { findRunningTask, isModKey, toBase64 } from "./helpers";
import { RootState } from "./store";

const BLANK_IMG = "data:image/gif;base64,R0lGODlhAQABAAAAACH5BAEKAAEALAAAAAABAAEAAAICTAEAOw==";

// Iframe Popup Component
function Url(props: { url: string }) {
    const u = useMemo(() => {
        try {
            return new URL(props.url)
        } catch (err) {
            console.log(err)
        }
    }, [props.url]);

    if (!u) {
        return <span className="font-mono hover:opacity-70 py-1">{props.url}</span>
    }

    return <span className="font-mono hover:opacity-70 py-1">
        <span className="underline font-bold text-orange-400">{u.protocol}{"//"}</span>
        <span className="underline font-bold text-orange-400">{u.hostname}</span>
        {u.port && <span className="underline font-bold text-orange-400">:{u.port}</span>}
        <span className="underline text-orange-400">{u.pathname}</span>
        <span className="underline text-orange-500">{u.search}</span>
        <span className="underline font-bold text-orange-500">{u.hash}</span>
    </span>
}

function Tags(props: { tags: string[] }) {
    const requestAddToSearch = (tag: string) => {
        const event = new CustomEvent("add-search-tag", { detail: tag });
        document.dispatchEvent(event);
    }
    return <div className="flex gap-1">
        {props.tags.map(tag => {
            return <div onClick={() => requestAddToSearch(tag)} className="font-mono cursor-pointer hover:text-gray-400 " key={tag}>#{tag}</div>
        })}
    </div>
}


interface Props {
    bmark: Bmark;
    style: React.CSSProperties;
    onSize: (width: number, height: number) => void
    onDelete?: () => void;
    onSave: (update: UpdateBmark) => void;
    onFetchMeta: () => void;
    setEditing: (val: boolean) => void;
    isEditing: boolean;
    focused: boolean;
    config: Config;
    tagList: string[];
    setShowIframePopup: (bmark?: Bmark) => void;
}

const urlPattern = /(https?:\/\/[^\s]+)/g;

function Description(props: { description: string }) {
    const description = useMemo(() => {
        return props.description.replace(urlPattern, '<a target="_blank" class="text-orange-500 hover:text-orange-300" href="$1">$1</a>');
    }, [props.description]);

    return <div dangerouslySetInnerHTML={{ __html: description }} />

}

function Bookmark(props: Props) {
    const [showExpand, setShowExpand] = useState(false);
    const [dragOverCover, setDragOverCover] = useState(false);
    const [dragOverIcon, setDragOverIcon] = useState(false);
    const [dragStart, setDragStart] = useState(false);

    const [hover, setHover] = useState(false);
    const taskQueue = useSelector((state: RootState) => state.taskQueue.value.queue);

    const currentTask = useMemo(() => {
        return findRunningTask(props.bmark.id);
    }, [taskQueue]);

    const [form, setForm] = useState({ ...props.bmark });
    const innerContainer = useRef<HTMLDivElement | null>();
    const outerContainer = useRef<HTMLDivElement | null>();

    const editing = props.isEditing;

    const {
        bmark,
    } = props;

    const tags = useMemo(() => {
        const t = bmark.tags.join(" #")
        if (t.length) {
            return "#" + t
        }
        return ""
    }, [bmark.tags]);

    function advertiseSize() {
        const rect = innerContainer.current?.getBoundingClientRect();
        if (rect) {
            props.onSize(rect?.width, rect?.height)
        }
    }

    function onExpand() {
        advertiseSize();
        setShowExpand(false);
    }

    useEffect(() => {
        const innerRect = innerContainer.current?.getBoundingClientRect();
        const outerRect = outerContainer.current?.getBoundingClientRect();
        if (!innerRect || !outerRect) {
            return
        }

        if (innerRect.height > outerRect.height) {
            setShowExpand(true);
        }

        // NOTE: auto size
        // if (innerRect) {
        //     props.onSize(innerRect.width, 400)
        // }
    }, []);

    const onDragStart = (e: React.DragEvent<HTMLElement>) => {
        if (e.dataTransfer.types.includes("Files")) {
            e.preventDefault()
            setDragStart(true)
        }
    }

    const onDragStartCover = (e: React.DragEvent<HTMLElement>) => {
        if (e.dataTransfer.types.includes("Files")) {
            e.preventDefault()
            setDragOverCover(true)
        }
    }

    const onDropIcon = (e: React.DragEvent<HTMLElement>) => {
        const file = e.dataTransfer.files.item(0);
        if (file) {
            e.preventDefault()
            setDragOverIcon(false);
            setDragStart(false)
            toBase64(file).then(b64 => {
                let updateBmark: UpdateBmark = {
                    id: bmark.id,
                    icon_b64: b64,
                };

                props.onSave(updateBmark);
            });
        }
    }

    const onDropCover = (e: React.DragEvent<HTMLElement>) => {
        const file = e.dataTransfer.files.item(0);
        if (file) {
            e.preventDefault()
            setDragOverCover(false);
            setDragStart(false)
            toBase64(file).then(b64 => {
                let updateBmark: UpdateBmark = {
                    id: bmark.id,
                    image_b64: b64,
                };

                props.onSave(updateBmark);
            });
        }
    }

    useEffect(() => {
        const onDragEnter = (e: DragEvent) => {
            if (!dragStart) {
                setDragStart(true);
            }
        }

        const onDragExit = (e: DragEvent) => {
            if (dragStart) {
                setDragStart(false);
            }
        }

        // document.addEventListener("dragenter", onDragEnter)
        // document.addEventListener("dragleave", onDragExit)
        return () => {
            document.removeEventListener("dragenter", onDragEnter)
            document.removeEventListener("dragleave", onDragExit)
        }
    }, [dragStart, dragOverCover])

    function onFetchMeta() {
        props.onFetchMeta?.();
    }

    function onSave() {
        let updateBmark: UpdateBmark = {
            id: form.id,
            url: form.url,
            title: form.title,
            description: form.description,
            tags: form.tags.join(",")
        };

        props.onSave(updateBmark);
    }

    function onEdit() {
        if (currentTask) {
            toast.error("cannot update while being processed.");
            return
        }

        props.setEditing(!editing);
    }

    function onDelete() {
        props.onDelete?.()
    }

    useEffect(() => {
        if (editing) {
            onExpand();
        }

        setForm({ ...props.bmark });
    }, [props.isEditing]);

    const inputStyle = "transition-all bg-gray-700 hover:bg-gray-600/90 focus:bg-gray-500 shadow-sm hover:shadow-inner focus:shadow-inner text-gray-100 rounded outline-0 p-1 px-2 text-gray-100 w-full";

    const interceptInputWithCb = (e: React.FormEvent<HTMLInputElement>, cb: () => void) => {
        // if (e.)

        cb();
    }

    const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
        if (e.key === "Enter" && !isModKey(e)) {
            e.preventDefault();
            onSave();
        }
    }

    return <div
        ref={ref => outerContainer.current = ref}
        id={"bmark-" + bmark.id.toString()}
        style={{ ...props.style }}
        onMouseOverCapture={(e) => {
            if (!hover) setHover(true)
        }}
        onMouseLeave={(e) => {
            if (hover) setHover(false)
        }}
        className="bmark-container relative text-wrap break-words group"
        onDragExit={() => setDragStart(false)}
        onDragEnd={() => setDragStart(false)}
        onDragLeave={() => setDragStart(false)}
        onDragEnter={onDragStart}
        onDragOver={onDragStart}
    >
        <div
            style={{ height: "calc(100% - 0.5rem)" }}
            className={`my-2 mx-1 ${props.focused ? 'bg-gray-600' : 'bg-gray-800'} h-auto rounded-lg overflow-hidden`}
        >
            <div ref={ref => innerContainer.current = ref}>
                <div
                    onDragExit={() => setDragOverCover(false)}
                    onDragEnd={() => setDragOverCover(false)}
                    onDragLeave={() => setDragOverCover(false)}
                    onDragEnter={onDragStartCover}
                    onDragOver={onDragStartCover}
                    onDrop={onDropCover}
                    className={"relative overflow-hidden" + (dragStart ? " bg-emerald-600 " : " ") + (dragOverCover ? " !bg-emerald-500	" : " ")}
                >
                    <a
                        target="_blank"
                        href={bmark.url}
                    >
                        <img
                            width={417}
                            height={300}
                            alt={bmark.title}
                            style={{ objectPosition: "50% 30%" }}
                            className={"object-cover z-10 w-full aspect-video " + (currentTask || dragStart ? "opacity-50" : "")}
                            src={bmark.image_id && !dragOverCover ? `/api/file/${bmark.image_id}` : BLANK_IMG}
                        />
                    </a>
                    {currentTask && <div className="absolute top-0 left-1 right-0 flex bottom-0 font-bold text-2xl leading-6 mb-1">
                        <span className="m-auto bg-pink-500 text-grey-100 p-2 rounded shadow-2xl">
                            Processing: {(currentTask.status as any)?.["Error"] ?? currentTask.status}
                        </span>
                    </div>}
                </div>
                <div className="px-3 py-2 flex gap-1" style={{ flexDirection: "column" }}>
                    {/* tags */}
                    {!editing && <div className="text-xs font-bold text-gray-300"><Tags tags={props.bmark.tags} /></div>}
                    {editing && <TagInput
                        onKeyDown={onKeyDown}
                        onValue={value => setForm({ ...form, tags: value })}
                        autoFocus
                        tagList={props.tagList}
                        defaultValue={form.tags.join(" ")}
                    />}

                    {/* title */}
                    {!editing && <div className="font-bold leading-6">
                        <a className={"bmark-url py-1 " + (!dragStart ? "hover:opacity-70" : "")} target="_blank" href={bmark.url}>
                            {(bmark.icon_id || dragStart) && <div
                                className={"inline-block rounded-sm mr-1 " + (dragStart ? " bg-emerald-600 p-2 px-3" : " ") + (dragOverIcon ? " !bg-emerald-500" : " ")}
                                onDragExit={() => setDragOverIcon(false)}
                                onDragEnd={() => setDragOverIcon(false)}
                                onDrop={onDropIcon}
                                onDragLeave={() => setDragOverIcon(false)}
                                onDragEnter={() => setDragOverIcon(true)}
                                onDragOver={() => setDragOverIcon(true)}
                            >
                                <img
                                    width={20}
                                    height={20}
                                    alt={bmark.title}
                                    className={
                                        "rounded-sm aspect-square self-center inline-block cursor-pointer hover:opacity-70 "
                                        + (dragStart ? "opacity-50" : "")
                                    }
                                    src={bmark.icon_id && !dragOverIcon ? `/api/file/${bmark.icon_id}` : BLANK_IMG}
                                    onClick={(e) => {
                                        e.preventDefault();
                                        e.stopPropagation();
                                        props.setShowIframePopup(bmark);
                                    }}
                                /></div>}
                            <span>{bmark.title}</span>
                        </a>
                    </div>}
                    {editing && <input
                        onKeyDown={onKeyDown}
                        onInput={e => setForm({ ...form, title: e.currentTarget.value })}
                        autoComplete="off"
                        value={form.title}
                        placeholder="Title"
                        className={inputStyle}
                    />}

                    {/* url */}
                    {!editing && <div className="text-xs"><a className="text-orange-500 hover:text-orange-300" target="_blank" href={bmark.url}><Url url={bmark.url} /></a></div>}
                    {editing && <input
                        onKeyDown={onKeyDown}
                        onInput={e => setForm({ ...form, url: e.currentTarget.value })}
                        type="url"
                        autoComplete="off"
                        autoCorrect="off"
                        value={form.url}
                        placeholder="Url"
                        className={inputStyle}
                    />}

                    {/* description */}
                    {!editing && <div
                        onClick={() => showExpand ? onExpand() : null}
                        className={"text-sm " + (showExpand ? "cursor-pointer" : "")}
                    >
                        <Description description={bmark.description} />
                    </div>}
                    {editing && <textarea
                        onKeyDown={onKeyDown}
                        onInput={e => setForm({ ...form, description: e.currentTarget.value })}
                        rows={3}
                        value={form.description}
                        placeholder="Description"
                        className={inputStyle}
                    />}

                    {showExpand && <div className="absolute bottom-1 right-2 mt-2 flex">
                        <div className="ml-auto text-md mt-2 opacity-100">
                            <Button onClick={onExpand}>
                                Expand
                            </Button>
                        </div>
                    </div>}

                    {(hover || props.focused) && !dragOverCover && <div className="absolute top-2 left-3 flex" style={{ flexDirection: "column" }}>
                        <div className="text-md mt-2 opacity-100">
                            <Button
                                className="px-4 py-1 font-bold"
                                onClick={onEdit}
                            >
                                {!editing ? "Edit" : "Cancel"}
                            </Button>
                        </div>
                        {editing && <div className="text-md mt-2 opacity-100 text-left">
                            <Button
                                className="px-4 py-1 font-bold bg-green-600 hover:bg-green-700"
                                onClick={onSave}
                            >
                                Save
                            </Button>
                        </div>}
                        {editing && <div className="text-md mt-2 opacity-100 text-left">
                            <ButtonConfirm
                                className="px-4 py-1 font-bold bg-blue-600 hover:bg-blue-700"
                                onClickConfirm={onFetchMeta}
                                confirmClassName={"px-4 py-1 font-bold bg-orange-600 hover:bg-orange-700"}
                                confirmChildren={"Are you sure?"}
                            >
                                Fetch meta
                            </ButtonConfirm>
                        </div>}
                    </div>}
                    {(hover || props.focused) && !dragOverCover && !editing && <div className="absolute top-2 right-3 flex">
                        <div className="ml-auto text-md mt-2 opacity-100">
                            <ButtonConfirm
                                className="px-4 py-1 font-bold"
                                onClickConfirm={onDelete}
                                confirmClassName={"px-4 py-1 bg-rose-800 hover:bg-rose-600 font-bold"}
                                confirmChildren={"Are you sure?"}
                            >
                                Delete
                            </ButtonConfirm>
                        </div>
                    </div>}
                </div>
            </div>
        </div>
    </div>
}

interface CreateBookmarkProps {
    defaultUrl?: string;
    defaultTitle?: string;
    defaultDescription?: string;
    defaultTags?: string[];
    onCreate: (bmark: BookmarkCreate) => void;

    className?: string;
    config: Config;
    tagList: string[];

    handleKeyDown: boolean;
}

export function CreateBookmark(props: CreateBookmarkProps) {
    const [url, setUrl] = useState(props.defaultUrl ?? "");
    const [title, setTitle] = useState(props.defaultTitle ?? "");
    const [descr, setDescr] = useState(props.defaultDescription ?? "");
    const [tags, setTags] = useState<string[]>(props.defaultTags ?? []);
    const [fetchMeta, setFetchMeta] = useState(true);
    const [asyncMeta, setAsyncMeta] = useState(true);
    const [useHeadless, setUseHeadless] = useState(true);


    const inputStyle = "transition-all bg-gray-700 hover:bg-gray-600/90 focus:bg-gray-500 shadow-sm hover:shadow-inner focus:shadow-inner text-gray-100 rounded outline-0 p-1 px-2 text-gray-100";

    const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
        if (!props.handleKeyDown) {
            return
        }
        if (e.key === "Enter" && !isModKey(e)) {
            e.preventDefault();
            onCreate();
        }
    }

    const onCreate = () => {
        if (!url) {
            return
        }

        props.onCreate({
            url,
            description: descr,
            tags: tags.join(","),
            title,
            async_meta: asyncMeta,
            no_headless: !useHeadless,
            no_meta: !fetchMeta,
        });
    };

    return <div className={"bmark-container relative text-wrap break-words "}>
        <div
            className={"my-2 mx-1 bg-gray-800 h-auto rounded-lg " + (props.className ?? "")}
        >
            {/*<img
                width={417}
                height={200}
                data-src={`/api/file/${bmark.image_id}`}
                className="object-cover w-full aspect-video"
                src={`/api/file/${bmark.image_id}`}
            />*/}
            <div className="px-3 py-2 flex gap-1" style={{ flexDirection: "column" }}>
                {/* url */}
                <input
                    onKeyDown={onKeyDown}
                    onInput={e => setUrl(e.currentTarget.value)}
                    type="url"
                    autoComplete="off"
                    autoCorrect="off"
                    value={url}
                    placeholder="Url"
                    className={inputStyle}
                    autoFocus
                />

                {/* tags */}

                <TagInput
                    onValue={setTags}
                    tagList={props.tagList}
                    defaultValue={tags.join(",")}
                    onKeyDown={onKeyDown}
                />

                {/* title */}
                <input
                    onKeyDown={onKeyDown}
                    onInput={e => setTitle(e.currentTarget.value)}
                    autoComplete="off"
                    value={title}
                    placeholder="Title"
                    className={inputStyle}
                />

                {/* description */}
                <textarea
                    onKeyDown={onKeyDown}
                    onInput={e => setDescr(e.currentTarget.value)}
                    rows={3}
                    value={descr}
                    placeholder="Description"
                    className={inputStyle}
                />

                <div className="my-2">
                    <div className="text-gray-100 font-bold">
                        Metadata setting:
                    </div>
                    <div className="text-gray-100 flex">
                        <input
                            id="fetch-meta"
                            onKeyDown={onKeyDown}
                            onChange={e => setFetchMeta(e.currentTarget.checked)}
                            type="checkbox"
                            autoComplete="off"
                            autoCorrect="off"
                            checked={fetchMeta}
                            placeholder="Tags"
                            className={inputStyle + " w-auto mr-2 text-left"}
                        />
                        <label htmlFor="fetch-meta" className="w-full">Fetch metadata</label>
                    </div>

                    <div className="text-gray-100 flex">
                        <input
                            id="async-meta"
                            onKeyDown={onKeyDown}
                            onChange={e => setAsyncMeta(e.currentTarget.checked)}
                            type="checkbox"
                            autoComplete="off"
                            autoCorrect="off"
                            disabled={!fetchMeta}
                            checked={asyncMeta}
                            placeholder="Tags"
                            className={inputStyle + " w-auto mr-2 text-left"}
                        />
                        <label htmlFor="async-meta" className="w-full">Fetch in background</label>
                    </div>

                    <div className="text-gray-100 flex">
                        <input
                            id="headless-meta"
                            onKeyDown={onKeyDown}
                            onChange={e => setUseHeadless(e.currentTarget.checked)}
                            type="checkbox"
                            autoComplete="off"
                            disabled={!fetchMeta}
                            autoCorrect="off"
                            checked={useHeadless}
                            placeholder="Tags"
                            className={inputStyle + " w-auto mr-2 text-left"}
                        />
                        <label htmlFor="headless-meta" className="w-full">Use headless browser</label>
                    </div>
                </div>

                <Button
                    className="px-4 py-1 font-bold bg-green-600 hover:bg-green-700 text-gray-100"
                    onClick={onCreate}
                >
                    Create
                </Button>
            </div>
        </div>
    </div>
}


export default Bookmark;
