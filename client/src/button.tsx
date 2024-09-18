import { JSXElementConstructor, ReactElement, useEffect, useRef, useState } from "react";
import { DELETE_CONFIRM_TIMEOUT_SEC } from "./vars";

interface Props {
    onClick?: (e: React.MouseEvent<HTMLButtonElement, MouseEvent>) => void;
    children?: ReactElement<any, JSXElementConstructor<any>> | string | undefined
    className?: string;
}

const Button = (props: Props) => {
    const className = props.className ?? "";
    return <button
        onClick={props?.onClick}
        className={"transition-all bg-gray-600 hover:bg-gray-700 shadow-sm hover:shadow-inner rounded px-2 " + className}
    >
        {props.children}
    </button>
}

interface ConfirmProps {
    onClickConfirm: (e: React.MouseEvent<HTMLButtonElement, MouseEvent>) => void;
    confirmChildren?: ReactElement<any, JSXElementConstructor<any>> | string | undefined;
    confirmClassName?: string;
    timeout?: number;
}

export const ButtonConfirm = (props: Props & ConfirmProps) => {
    const [confirming, setConfirming] = useState(false);
    const timer = useRef<any>();

    const onFirstClick = (e: React.MouseEvent<HTMLButtonElement, MouseEvent>) => {
        setConfirming(true);
        clearTimeout(timer.current);
        timer.current = setTimeout(() => {
            setConfirming(false);
        }, props.timeout ?? (DELETE_CONFIRM_TIMEOUT_SEC * 1000));

        props.onClick?.(e);
    };

    const onSecondClick = (e: React.MouseEvent<HTMLButtonElement, MouseEvent>) => {
        clearTimeout(timer.current);
        setConfirming(false);
        props.onClickConfirm(e);
    };

    useEffect(() => {
        return () => {
            clearTimeout(timer.current);
        }
    }, []);

    if (confirming) {
        return <Button onClick={onSecondClick} className={props.confirmClassName}>
            {props.confirmChildren ?? props.children}
        </Button>
    }

    return <Button onClick={onFirstClick} className={props.className}>
        {props.children}
    </Button>

};

export default Button;
