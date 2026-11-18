export const motionEaseOut = [0.22, 1, 0.36, 1] as const;
export const motionEaseIn = [0.4, 0, 1, 1] as const;
export const motionEnter = { duration: 0.28, ease: motionEaseOut };
export const motionExit = { duration: 0.14, ease: motionEaseIn };
export const motionSpring = { type: "spring", duration: 0.32, bounce: 0 } as const;
