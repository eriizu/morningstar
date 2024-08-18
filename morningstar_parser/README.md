# GTFS experiment with rust

## Why

Bad PDF timetables, bad transit apps, complicated bus lines. I want something better.
Something simpler. I want people to save time and avoid suprises when going to
and from work. When planing to take the bus round trip to that nice shop this other city has
and being able to know how much time they have in-between buses so as not to miss the next
on and end up waiting an hour for the one after.
It's about quality of life using the infrastructure we already have.

The world could be simpler.

I already started working on a tool that checked some of the boxes. A CLI my students
can call on their work sessions, to know when is the bus coming after classes,
rather than guessing (what they used to do before).

The issue is that this tool I made, relies on a dataset I created by scrapping a PDF
timetable by hand. I don't want to do that everytime a timetable is published.
Hence this project: it's about taking a GTFS "stream" and extracting every trip
that I want to integrate into my dataset.

Once it will be done, updating the set will be as easy as:
- downloading the new GTFS "stream";
- runing my program;
- that's it, profit!

There is still a lot to experiment with and some architecture concerns I want to work on.
This is what this repo is for.

## The future

- improving existing CLI
- creating a web interface, placing QRcodes at bus stops to help any passerby taking the bus
- integrating it into a student dashboard with other qol features
  - such as deadline tracking, events, announcements
