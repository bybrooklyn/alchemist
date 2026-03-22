import { Children, useEffect } from "react";
import type { ReactNode } from "react";

interface TransitionGroupProps {
    children?: ReactNode;
    component?: keyof JSX.IntrinsicElements | ((props: { children?: ReactNode }) => JSX.Element);
}

interface TransitionProps {
    children?: ReactNode | (() => ReactNode);
    onEnter?: (node?: unknown, isAppearing?: boolean) => void;
    onExit?: (node?: unknown) => void;
}

export function TransitionGroup({
    children,
    component: Component = "span",
}: TransitionGroupProps) {
    return <Component>{children}</Component>;
}

export function Transition({ children, onEnter }: TransitionProps) {
    useEffect(() => {
        onEnter?.(undefined, true);
    }, [onEnter]);

    return typeof children === "function" ? <>{children()}</> : <>{Children.only(children)}</>;
}
