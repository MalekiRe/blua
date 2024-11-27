local app = ...
function my_system(query_1)
    local awa = nil
    for transform in query_1:iter() do
        if awa == nil then
            awa = transform
        end
        print(transform)
    end
    --can use outside of iteration scope, just not outside of function scope
    print(awa.translation)

    awa = nil
end

function second_system(my_q)
    for t, stretch in my_q:iter() do
        print(t.translation.x)
        print(stretch)
        stretch.x = t.translation.x
    end
end

local third_params = { {bevy_transform.components.transform.Transform.ref}, {simple_test.Stretch.mut} }
function third(q1, q2)
    for t in q1:iter() do
        print(t.translation.z)
    end

    for s in q2:iter() do
        --print(s.x)
        print(s:get_sum())
        print("and sum with is")
        print(s:get_sum_with(4370.0))
    end
end

local thing = nil

function try_do_bad(q)
    if thing == nil then
        for t in q:iter() do
            print("setting")
            thing = t
        end
    else
        thing.translation.x = 0.1
        print(thing)
    end
end

--app:register_system(my_system, { {bevy_transform.components.transform.Transform.mut} })
--app:register_system(second_system, { {bevy_transform.components.transform.Transform.mut, simple_test.Stretch.mut} })
app:register_system(third, third_params)
--app:register_system(try_do_bad, { {bevy_transform.components.transform.Transform.mut} })
print("hello world!")
