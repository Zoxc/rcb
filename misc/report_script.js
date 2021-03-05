console.log(DATA);

let summary = `<table><tr><th rowspan="2">Benchmark</th>`;

for (let i = 0; i < DATA.benchs[0].builds.length; i++) {
    const config = DATA.benchs[0].builds[i];
    summary += `<th colspan="${i > 0 ? 2 : 1}">${config.build}</th>`
}

for (let i = 0; i < DATA.benchs[0].builds.length; i++) {
    const config = DATA.benchs[0].builds[i];
    summary += `<th colspan="${i > 0 ? 2 : 1}">${config.build}</th>`
}

summary += "</tr><tr>";

for (let i = 0; i < DATA.benchs[0].builds.length; i++) {
    summary += `<th>Time</th>`;
    if (i > 0) {
        summary += `<th>%</th>`;
    }
}

for (let i = 0; i < DATA.benchs[0].builds.length; i++) {
    summary += `<th>Memory</th>`;
    if (i > 0) {
        summary += `<th>%</th>`;
    }
}

summary += "</tr>";

function change_class(c) {
    if (c > 1) {
        return 'positive';
    } else if (c > 0.1) {
        return 'slightly-positive';
    } else if (c < -1) {
        return 'negative';
    } else if (c < -0.1) {
        return 'slightly-negative';
    } else {
        return '';
    }
}

function average_by(array, f = (a) => a) {
    return array.reduce((accumulator, current) => accumulator + f(current), 0) / array.length;
}

function max_rss(time) {
    console.log("max_rss", time);

    //return 10;

    return time.reduce((accumulator, current) => {
        console.log("reduce2", current);
        let rss = Math.max(parseInt(current.after_rss), parseInt(current.before_rss));
        console.log("reduce", rss);
        return Math.max(accumulator, rss);
    }, 0);
}

for (const bench of DATA.benchs) {

    summary += `<tr><th>${bench.name}</th>`;

    let first = average_by(bench.builds[0].time);

    for (let i = 0; i < bench.builds.length; i++) {
        const config = bench.builds[i];
        let average = average_by(config.time);
        summary += `<td>${average.toFixed(4)}s</td>`
        if (i > 0) {
            let change = (average / first - 1) * 100;
            summary += `<td class=${change_class(change)}> ${change.toFixed(2)}%</td>`;
        }
    }

    let first_rss = average_by(bench.builds[0].times, time => max_rss(time));

    for (let i = 0; i < bench.builds.length; i++) {
        const config = bench.builds[i];
        let average = average_by(config.times, time => max_rss(time));
        summary += `<td>${average.toFixed(2)} MiB</td>`
        if (i > 0) {
            let change = (average / first_rss - 1) * 100;
            summary += `<td class=${change_class(change)}> ${change.toFixed(2)}%</td>`;
        }
    }

    summary += "</tr>";

}

summary += "</table>";

document.body.innerHTML = `<div><h1>${document.title}</h1>${summary}</div>`