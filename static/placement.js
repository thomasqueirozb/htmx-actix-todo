function findClosestLi(y, target, cardId) {
    if (target.tagName === 'LI') {
        target = target.parentElement;
    }

    while (target) {
        if (target.tagName === 'UL') {
            break;
        }
        target = target.parentElement;
    }

    if (target === null) {
        return null;
    }

    let closestLi = null;
    let closestLiIdx = null;
    let closestDistance = Number.MAX_VALUE;
    let idx = 0;
    let cardIdx = null;

    // Traverse the <ul> children and find the closest <li>
    for (let liElement of target.children) {
        if (liElement.tagName !== 'LI') {
            continue;
        }

        if (liElement.id == cardId) {
            cardIdx = idx;
        }

        let rect = liElement.getBoundingClientRect()

        // Calculate the vertical distance between the mouse pointer and the middle of the li
        let distance = Math.abs(y - (rect.top + rect.height / 2));

        // Update closestLi if this is closer
        if (distance < closestDistance) {
            closestLi = liElement;
            closestLiIdx = idx;
            closestDistance = distance;
        }

        idx++;
    }

    if (!closestLi) {
        return null;
    }

    // Moving inside the same list
    if (cardIdx !== null && cardIdx < closestLiIdx) {
        closestLiIdx--;
    }

    return {
        li: closestLi,
        idx: closestLiIdx,
    }
}

function determinePlacement(event, card) {
    let y = event.clientY
    let closestLiData = findClosestLi(y, event.target, card.id);

    if (!closestLiData) {
        return null;
    }

    let closestLi = closestLiData.li;
    let closestLiIdx = closestLiData.idx;

    let rect = closestLi.getBoundingClientRect();
    let placeBefore = event.clientY < rect.top + rect.height / 2

    return {
        closestLi: closestLi,
        idx: closestLiIdx + (placeBefore ? 0 : 1),
        placeBefore: placeBefore,
    };

}
