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
    summary += `<th class="r">Time</th>`;
    if (i > 0) {
        summary += `<th class="r">%</th>`;
    }
}

for (let i = 0; i < DATA.benchs[0].builds.length; i++) {
    summary += `<th class="r">Memory</th>`;
    if (i > 0) {
        summary += `<th class="r">%</th>`;
    }
}

summary += "</tr>";

function change(average, first, b) {
    if (!b) {
        return "";
    }
    let change = (average / first - 1) * 100;
    if (average == first) {
        change = 0;
    }
    let klass = change_class(change);
    if (change == Infinity) {
        change = "---------";
    } else {
        change = change.toFixed(2) + '%';
    }
    return `<td class=${klass}>${change}</td>`;
}

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
    return time.reduce((accumulator, current) => {
        let rss = Math.max(parseFloat(current.after_rss), parseFloat(current.before_rss));
        return Math.max(accumulator, rss);
    }, 0);
}

for (const bench of DATA.benchs) {

    summary += `<tr><th><a href="#${bench.name}">${bench.name}</a></th>`;

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

let benchs = "";


for (const bench of DATA.benchs) {
    benchs += `<h3 id="${bench.name}">Details of <b>${bench.name}</b></h3><table><tr><th rowspan="2">Stage</th>`

    for (let i = 0; i < bench.builds.length; i++) {
        const config = bench.builds[i];
        benchs += `<th colspan="${i > 0 ? 2 : 1}">${config.build}</th>`
    }

    for (let i = 0; i < bench.builds.length; i++) {
        const config = bench.builds[i];
        benchs += `<th colspan="${i > 0 ? 2 : 1}">${config.build}</th>`
    }

    benchs += "</tr><tr>";

    for (let i = 0; i < bench.builds.length; i++) {
        benchs += `<th class="r">Time</th>`;
        if (i > 0) {
            benchs += `<th class="r">%</th>`;
        }
    }

    for (let i = 0; i < bench.builds.length; i++) {
        benchs += `<th class="r">Memory</th>`;
        if (i > 0) {
            benchs += `<th class="r">%</th>`;
        }
    }

    benchs += "</tr>";

    let data = bench.builds.map(build => {
        let entries = {};

        for (const instance of build.times) {
            for (const data of instance) {
                if (entries[data.name] === undefined) {
                    entries[data.name] = [];
                }
                entries[data.name].push(data);
            }
        }

        let entries_avg = {};

        for (const entry in entries) {
            entries_avg[entry] = {
                time: average_by(entries[entry], entry => entry.time),
                rss: average_by(entries[entry], entry => parseFloat(entry.after_rss)),
            };
        }

        return entries_avg;
    });

    let times = bench.builds[0].times[0].map(entry => entry.name).filter(entry => {
        return data.every(build => {
            return build[entry] != undefined;
        });
    });

    for (let i = 0; i < times.length; i++) {
        const config = times[i];
        benchs += `<tr><th class="event">${config}</th>`;

        let first = data[0][config].time;

        for (let j = 0; j < data.length; j++) {
            const build = data[j];
            let average = build[config].time;
            benchs += `<td>${average.toFixed(4)}s</td>`;
            benchs += change(average, first, j > 0);
        }

        let first_rss = data[0][config].rss;

        for (let j = 0; j < data.length; j++) {
            const build = data[j];
            let average = build[config].rss;
            benchs += `<td>${average.toFixed(2)} MiB</td>`;
            benchs += change(average, first_rss, j > 0);
        }

        benchs += `</tr>`
    }

    benchs += `</table>`;
}

function linearize(keys, path, out) {
    for (const key in keys) {
        let new_path = path.concat([key]);
        if (typeof keys[key] === 'object') {
            linearize(keys[key], new_path, out)
        } else {
            out.push([new_path, keys[key]]);
        }
    }
}

for (const build of DATA.builds) {
    let out = [];
    linearize(build.config, [], out);
    build.config_linearized = out;
}

let common_opts = DATA.builds[0].config_linearized.filter(opt => {
    return DATA.builds.every(build => {
        return build.config_linearized.find(build_opt => JSON.stringify(opt) == JSON.stringify(build_opt)) !== undefined
    });
});

if (DATA.builds.length == 1) {
    common_opts = []
}

summary += `<div class="build-container"><div class="build"><h3>Common build options</h3>`;
for (const opt of common_opts) {
    summary += `<div class="split"><p>${opt[0].join(".")}:</p><p><b>${opt[1]}</b></p></div>`;
}
summary += `</div>`;

for (let i = 0; i < DATA.builds.length; i++) {
    let build = DATA.builds[i];
    summary += `<div class="build"><h3>Build <b>${build.name}</b></h3>`;
    summary += `<div class="split"><p>Git commit:</p><p><b>${build.commit_short}</b></p></div>`;
    summary += `<div class="split"><p>Git branch:</p><p><b>${build.branch}</b></p></div>`;
    summary += `<div class="split"><p>Triple:</p><p><b>${build.triple}</b></p></div>`;
    summary += `<div class="split"><p>From repo:</p><p><b>${build.repo}</b> at ${build.repo_path}</p></div>`;

    let opts = build.config_linearized.filter(opt => common_opts.find(common_opt => JSON.stringify(opt) == JSON.stringify(common_opt)) === undefined);

    if (opts.length > 0) {

        summary += `<h4>Additional build options:</h4>`;

        for (const opt of opts) {
            summary += `<div class="split"><p>${opt[0].join(".")}:</p><p><b>${opt[1]}</b></p></div>`;

        }
    }

    summary += `</div>`;
}

summary += `</div>`;

let title = `Benchmark result for `;

for (let i = 0; i < DATA.benchs[0].builds.length; i++) {
    if (i > 0) {
        title += ` vs. `;
    }
    title += `<b>${DATA.benchs[0].builds[i].build}</b>`;
}


document.body.innerHTML = `<div><h1>${title}</h1>${summary}${benchs}</div>`;
